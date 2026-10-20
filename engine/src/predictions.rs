//! Held-out predictions (L11): record the commitments an adopted candidate made, and score them.
//!
//! `metaform apply` writes every adopted candidate that carried a `predict` field to
//! `<project>/.metaform/predictions.tsv`. The format is one header line plus one tab-separated line
//! per prediction: `request_id`, candidate name, predicted object id, predicted label (`pos` / `neg`),
//! and the candidate's extension as a comma-separated id list. Rows are written in response order and
//! deduplicated on `(request_id, name, id)`, so applying the same response twice does not duplicate a
//! commitment and the same input always produces the same bytes (LF line endings, sorted extension).
//!
//! `metaform check-predictions --project <dir>` reads that record and compares each prediction with the
//! object's label in the current state. A label is "strong" under the same threshold the kernel uses:
//! `recommended_theta()` from the vote counts, then `strong(theta)` -> a positive needs
//! `yes - no >= theta`, a negative `no - yes >= theta`. Predictions whose object has no strong label
//! yet are counted in `open`, not scored. The command is read-only and prints one deterministic JSON
//! line: `{"checked":n,"correct":n,"wrong":[{"name":...,"id":...,"predicted":...,"actual":...}],"open":n}`.
//!
//! This is the enforcement half of L11: a candidate may not be adopted on fit alone. It must name an
//! object it was not built from and commit to that object's label before the label is measured.

use std::collections::HashSet;
use std::path::Path;

use metaform::bit;
use metaform::propose::{self, Accepted};

/// The header line of `.metaform/predictions.tsv`.
pub const HEADER: &str = "request_id\tname\tid\tpredicted\textension";

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
            "{request_id}\t{}\t{}\t{}\t{}",
            candidate.name,
            prediction.id,
            prediction.label.as_str(),
            extension(&candidate.ids)
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
}

/// Read the record. A missing file is an empty record; a malformed row is skipped, because the file
/// is a derived record and only rows written by [`record`] are guaranteed to be well formed.
fn read_records(path: &Path) -> std::io::Result<Vec<Record>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line == HEADER {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 {
            continue;
        }
        let Ok(id) = fields[2].parse::<usize>() else {
            continue;
        };
        out.push(Record {
            name: fields[1].to_string(),
            id,
            predicted: fields[3].to_string(),
        });
    }
    Ok(out)
}

/// `metaform check-predictions --project <dir>`
///
/// Read-only. The status is the one `state need` and `suggest` use: the recommended `theta` from the
/// recorded votes, then `strong(theta)`. A prediction is scored only once its object has a strong
/// label; until then it is `open`.
pub fn cmd(args: &[String]) -> i32 {
    let root = crate::project_root(args).join(".metaform");
    let Some(st) = crate::load_state(&root.join("state.json"), "check-predictions") else {
        return 2;
    };
    let records = match read_records(&root.join("predictions.tsv")) {
        Ok(records) => records,
        Err(e) => {
            eprintln!(
                "metaform check-predictions: cannot read `{}`: {e}",
                root.join("predictions.tsv").display()
            );
            return 2;
        }
    };

    let explorer = st.explorer();
    let theta = explorer.recommended_theta();
    let (sp, sn) = explorer.strong(theta);
    let mut checked = 0usize;
    let mut correct = 0usize;
    let mut open = 0usize;
    let mut wrong: Vec<String> = Vec::new();
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
    }

    println!(
        "{{\"checked\":{checked},\"correct\":{correct},\"wrong\":[{}],\"open\":{open}}}",
        wrong.join(",")
    );
    0
}
