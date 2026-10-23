//! Held-out predictions (L11): record the commitments an adopted candidate made, and score them.
//!
//! `metaform apply` writes every adopted candidate that carried a `predict` field to
//! `<project>/.metaform/predictions.tsv`. The format is one header line plus one tab-separated line
//! per prediction: `request_id`, candidate name, predicted object id, predicted label (`pos` / `neg`),
//! the candidate's extension as a comma-separated id list, and the extension of the `rival` the
//! candidate named (empty when it named none). Rows are written in response order and deduplicated on
//! `(request_id, name, id)`, so applying the same response twice does not duplicate a commitment and
//! the same input always produces the same bytes (LF line endings, sorted extensions).
//!
//! The `rival` column is the last one, so a file written before it existed still loads: a row with
//! four fields (the pre-extension shape) or five fields (header, name, id, label, extension) is read
//! with an empty rival, and the five-column header is recognised as a header rather than data.
//!
//! `metaform check-predictions --project <dir>` reads that record and compares each prediction with the
//! object's label in the current state. A label is "strong" under the same threshold the kernel uses:
//! `recommended_theta()` from the vote counts, then `strong(theta)` -> a positive needs
//! `yes - no >= theta`, a negative `no - yes >= theta`. Predictions whose object has no strong label
//! yet are counted in `open`, not scored. The command is read-only and prints one deterministic JSON
//! line:
//! `{"checked":n,"correct":n,"wrong":[{"name":...,"id":...,"predicted":...,"actual":...}],"open":n,"discriminating":n}`.
//! `discriminating` counts the checked predictions whose row carries a non-empty rival that disagrees
//! with the candidate's extension on the predicted object: those are the bets only one of the two
//! rules could have made. A wrong discriminating prediction is still a `wrong` entry, as before.
//!
//! This is the enforcement half of L11: a candidate may not be adopted on fit alone. It must name an
//! object it was not built from and commit to that object's label before the label is measured. When
//! it names a `rival`, the commitment must also land where the two rules disagree, so a coherent but
//! false rule cannot pass on a prediction that any nearby rule would make.

use std::collections::HashSet;
use std::path::Path;

use metaform_kernel::bit;
use metaform_propose::{self as propose, Accepted};

/// The header line of `.metaform/predictions.tsv`.
pub const HEADER: &str = "request_id\tname\tid\tpredicted\textension\trival\tscore\tpair";

/// The header written before the `rival` column existed. It still has to be skipped as a header, not
/// read as a malformed data row.
const LEGACY_HEADER: &str = "request_id\tname\tid\tpredicted\textension";

/// The header written before the `score` column existed.
const RIVAL_HEADER: &str = "request_id\tname\tid\tpredicted\textension\trival";

/// The header written before the `pair` column existed.
const SCORE_HEADER: &str = "request_id\tname\tid\tpredicted\textension\trival\tscore";

/// The extension as a deterministic comma-separated id list (ascending, no duplicates).
fn extension(ids: &[usize]) -> String {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<String>>()
        .join(",")
}

/// Read one comma-separated id cell. An empty cell is an empty extension; an unparsable cell is a
/// malformed row (`None`), which [`read_records`] skips. The field is never written with spaces, but
/// trimming keeps a hand-edited record readable.
fn parse_extension(cell: &str) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in cell.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        out.push(part.parse::<usize>().ok()?);
    }
    Some(out)
}

/// Append the predictions of `candidates` that are not yet in the record.
///
/// `candidates` are the adopted candidates, in response order. A missing file starts with [`HEADER`];
/// an existing file keeps every byte before the appended rows. Nothing is written when every row is a
/// duplicate, so replaying an unchanged response leaves the file byte-for-byte identical.
pub fn record(path: &Path, request_id: &str, candidates: &[Accepted]) -> std::io::Result<()> {
    if !candidates.iter().any(|c| c.prediction.is_some()) {
        return Ok(());
    }

    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };

    // The dedupe key is (request_id, name, id); the extension and label are not part of it, so a
    // commitment cannot be rewritten under the same key.
    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    for line in existing.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() >= 3 {
            seen.insert((
                fields[0].to_string(),
                fields[1].to_string(),
                fields[2].to_string(),
            ));
        }
    }

    let mut rows: Vec<String> = Vec::new();
    for candidate in candidates {
        let Some(prediction) = &candidate.prediction else {
            continue;
        };
        let key = (
            request_id.to_string(),
            candidate.name.clone(),
            prediction.id.to_string(),
        );
        if !seen.insert(key) {
            continue;
        }
        rows.push(format!(
            "{request_id}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            candidate.name,
            prediction.id,
            prediction.label.as_str(),
            extension(&candidate.ids),
            // The rival column is always present and is empty when the candidate named no rival, so
            // every row has six fields whether or not the bet was discriminating.
            candidate
                .rival
                .as_deref()
                .map(extension)
                .unwrap_or_default(),
            // The score is empty when the candidate carried none; an abstention cannot be recorded
            // without one, so a non-empty abstain always has a number beside it.
            candidate.score.map(|s| s.to_string()).unwrap_or_default(),
            // The falsifier pair, when the candidate staked itself on one. Empty for prose, which
            // cannot be settled, and for candidates that carried no falsifier at all.
            candidate
                .falsifier_pair
                .map(|(a, b)| format!("{a},{b}"))
                .unwrap_or_default()
        ));
    }
    if rows.is_empty() {
        return Ok(());
    }

    let mut out = if existing.trim().is_empty() {
        format!("{HEADER}\n")
    } else {
        existing
    };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    for row in rows {
        out.push_str(&row);
        out.push('\n');
    }
    std::fs::write(path, out)
}

/// One recorded prediction, with the fields `check-predictions` needs.
struct Record {
    name: String,
    id: usize,
    predicted: String,
    /// The candidate's extension. Empty on a row written before that column existed.
    extension: Vec<usize>,
    /// The rival's extension. Empty when the row named no rival, and also for every row written
    /// before the column existed, which is what `discriminating` counts.
    rival: Vec<usize>,
    /// The falsifier pair, when the candidate staked itself on one. `None` for prose, for no
    /// falsifier, and for every row written before the column existed.
    pair: Option<(usize, usize)>,
}

/// Read the record. A missing file is an empty record; a malformed row is skipped, because the file
/// is a derived record and only rows written by [`record`] are guaranteed to be well formed.
///
/// A row may carry four fields (request, name, id, label), five (plus the extension), six (plus the
/// rival), seven (plus the score), or eight (plus the falsifier pair). A missing trailing column is
/// read as empty, so a file written by any earlier version keeps loading.
fn read_records(path: &Path) -> std::io::Result<Vec<Record>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty()
            || line == HEADER
            || line == LEGACY_HEADER
            || line == RIVAL_HEADER
            || line == SCORE_HEADER
        {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }
        let Ok(id) = fields[2].parse::<usize>() else {
            continue;
        };
        let extension = match parse_extension(fields.get(4).copied().unwrap_or("")) {
            Some(extension) => extension,
            None => continue,
        };
        let rival = match parse_extension(fields.get(5).copied().unwrap_or("")) {
            Some(rival) => rival,
            None => continue,
        };
        // A pair cell holds exactly two ids. Anything else is a hand-edit, and the row keeps loading
        // with no pair rather than taking the file down: the pair is the newest and least load-bearing
        // column.
        let pair = match fields.get(7).copied().unwrap_or("").trim() {
            "" => None,
            cell => match parse_extension(cell) {
                Some(ids) if ids.len() == 2 => Some((ids[0], ids[1])),
                _ => None,
            },
        };
        out.push(Record {
            name: fields[1].to_string(),
            id,
            predicted: fields[3].to_string(),
            extension,
            rival,
            pair,
        });
    }
    Ok(out)
}

/// Read the project and score every recorded prediction, returning one deterministic JSON line.
///
/// This is the non-CLI entry point shared by `metaform check-predictions` and the MCP
/// `check_predictions` tool. Read-only. The status is the one `state need` and `suggest` use: the
/// recommended `theta` from the recorded votes, then `strong(theta)`. A prediction is scored only
/// once its object has a strong label; until then it is `open`.
pub fn check(project: &Path) -> Result<String, String> {
    let root = project.join(".metaform");
    let st = crate::state::State::load(&root.join("state.json"))?;
    let predictions_path = root.join("predictions.tsv");
    let records = read_records(&predictions_path)
        .map_err(|e| format!("cannot read `{}`: {e}", predictions_path.display()))?;

    let explorer = st.explorer();
    let theta = explorer.recommended_theta();
    let (sp, sn) = explorer.strong(theta);
    let mut checked = 0usize;
    let mut correct = 0usize;
    let mut open = 0usize;
    let mut discriminating = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    let mut refuted: Vec<String> = Vec::new();
    for record in &records {
        let actual = if record.id < explorer.n && sp & bit(record.id) != 0 {
            Some("pos")
        } else if record.id < explorer.n && sn & bit(record.id) != 0 {
            Some("neg")
        } else {
            None
        };
        match actual {
            Some(actual) => {
                checked += 1;
                // A discriminating bet is one the candidate and its rival did not both make: the
                // recorded extensions disagree on the very object that was predicted.
                let ext_has = record.extension.contains(&record.id);
                let rival_has = record.rival.contains(&record.id);
                if !record.rival.is_empty() && ext_has != rival_has {
                    discriminating += 1;
                }
                if record.predicted == actual {
                    correct += 1;
                } else {
                    wrong.push(format!(
                        "{{\"name\":{},\"id\":{},\"predicted\":{},\"actual\":{}}}",
                        propose::json_string(&record.name),
                        record.id,
                        propose::json_string(&record.predicted),
                        propose::json_string(actual)
                    ));
                }
            }
            None => open += 1,
        }

        // The falsifier pair, settled a second time. A pair asserts that the member inside the
        // extension is positive and the member outside it is negative; a label that arrived after
        // adoption and says otherwise is the commitment firing. Kept apart from `wrong`, which is
        // about the single object the candidate chose to predict: a candidate can be right about its
        // prediction and still have been wrong about the pair it named.
        if let Some((a, b)) = record.pair {
            for id in [a, b] {
                let actual_pos = if id < explorer.n && sp & bit(id) != 0 {
                    Some(true)
                } else if id < explorer.n && sn & bit(id) != 0 {
                    Some(false)
                } else {
                    None
                };
                if actual_pos.is_some_and(|m| m != record.extension.contains(&id)) {
                    refuted.push(format!(
                        "{{\"name\":{},\"pair\":[{a},{b}],\"fired_on\":{id}}}",
                        propose::json_string(&record.name)
                    ));
                    break;
                }
            }
        }
    }

    Ok(format!(
        "{{\"checked\":{checked},\"correct\":{correct},\"wrong\":[{}],\"open\":{open},\"discriminating\":{discriminating},\"refuted\":[{}]}}",
        wrong.join(","),
        refuted.join(",")
    ))
}
