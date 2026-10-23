//! Metrics for judging a three-valued judge.
//!
//! Three subcommands, each over a plain file so the numbers can be recomputed without a model:
//!
//! - `verdict <rows.tsv>`: precision on answered items with a Wilson interval, the unknown rate, the
//!   wrong-answered rate, selective accuracy at a coverage level, and AURC.
//! - `bets <predictions.tsv>`: the divergence rate of recorded predictions, i.e. the share of bets
//!   placed where the candidate and its rival disagree.
//! - `readings --vocab <tsv> --readings <tsv>`: complement coverage and axis equivalence rate.
//!
//! These live in the engine rather than in a script because they must share the repository's build,
//! its tests and its gates; a parallel implementation in another language drifts.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;

/// Wilson score interval for a binomial proportion.
fn wilson(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 0.0);
    }
    let z = 1.96_f64;
    let p = k as f64 / n as f64;
    let denom = 1.0 + z * z / n as f64;
    let centre = (p + z * z / (2.0 * n as f64)) / denom;
    let half = z * ((p * (1.0 - p) / n as f64) + z * z / (4.0 * (n as f64).powi(2))).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

#[derive(Debug, Clone)]
struct Row {
    system: String,
    verdict: String,
    truth: String,
    score: Option<f64>,
}

fn read_table(path: &Path, required: &[&str]) -> Result<Vec<BTreeMap<String, String>>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| format!("{} is empty", path.display()))?
        .split('\t')
        .collect();
    for name in required {
        if !header.contains(name) {
            return Err(format!("{} needs a `{name}` column", path.display()));
        }
    }
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        let mut map = BTreeMap::new();
        for (i, name) in header.iter().enumerate() {
            map.insert(name.to_string(), cells.get(i).unwrap_or(&"").to_string());
        }
        rows.push(map);
    }
    Ok(rows)
}

fn verdict_rows(path: &Path) -> Result<Vec<Row>, String> {
    read_table(path, &["system", "item", "verdict", "truth"])?
        .into_iter()
        .map(|m| {
            let score = m
                .get("score")
                .filter(|s| !s.is_empty())
                .map(|s| s.parse::<f64>())
                .transpose()
                .map_err(|e| format!("score is not a number: {e}"))?;
            Ok(Row {
                system: m["system"].clone(),
                verdict: m["verdict"].clone(),
                truth: m["truth"].clone(),
                score,
            })
        })
        .collect()
}

fn risk_curve(rows: &[Row]) -> (Vec<(f64, f64)>, f64) {
    let mut ordered = rows.to_vec();
    ordered.sort_by(|a, b| {
        b.score
            .unwrap_or(0.0)
            .partial_cmp(&a.score.unwrap_or(0.0))
            .unwrap()
    });
    let total = ordered.len();
    let mut points = Vec::with_capacity(total);
    let mut errors = 0usize;
    for (i, row) in ordered.iter().enumerate() {
        if row.verdict == "unknown" || row.verdict != row.truth {
            errors += 1;
        }
        points.push(((i + 1) as f64 / total as f64, errors as f64 / (i + 1) as f64));
    }
    let aurc = points.iter().map(|(_, r)| r).sum::<f64>() / total as f64;
    (points, aurc)
}

/// The verdict report, one entry per system.
pub fn verdict_report(path: &Path) -> Result<serde_json::Value, String> {
    let rows = verdict_rows(path)?;
    let mut systems: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in rows {
        systems.entry(row.system.clone()).or_default().push(row);
    }
    let mut out = serde_json::Map::new();
    for (system, sub) in systems {
        let n = sub.len();
        let answered: Vec<&Row> = sub.iter().filter(|r| r.verdict != "unknown").collect();
        let correct = answered.iter().filter(|r| r.verdict == r.truth).count();
        let (lo, hi) = wilson(correct, answered.len());
        let (points, aurc) = risk_curve(&sub);
        let at80 = (0.8 * n as f64).ceil() as usize;
        let selective80 = points
            .get(at80.saturating_sub(1))
            .map(|(_, risk)| 1.0 - risk)
            .unwrap_or(0.0);
        out.insert(
            system,
            json!({
                "n": n,
                "answered": answered.len(),
                "unknown_rate": round((n - answered.len()) as f64 / n as f64),
                "precision_on_answered": round(correct as f64 / answered.len() as f64),
                "precision_ci": [round(lo), round(hi)],
                "wrong_answered_rate": round((answered.len() - correct) as f64 / n as f64),
                "selective_accuracy_at_80": round(selective80),
                "aurc": round(aurc),
            }),
        );
    }
    Ok(serde_json::Value::Object(out))
}

fn round(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Bet divergence: rows whose predicted object is classified differently by candidate and rival.
pub fn bets_report(path: &Path) -> Result<serde_json::Value, String> {
    let rows = read_table(path, &["id", "predicted", "extension"])?;
    let mut total = 0usize;
    let mut divergent = 0usize;
    for row in rows {
        total += 1;
        let rival = row.get("rival").cloned().unwrap_or_default();
        if rival.trim().is_empty() {
            continue;
        }
        let id = row["id"].trim();
        let in_ext = row["extension"].split(',').any(|x| x.trim() == id);
        let in_rival = rival.split(',').any(|x| x.trim() == id);
        if in_ext != in_rival {
            divergent += 1;
        }
    }
    Ok(json!({
        "bets": total,
        "divergent": divergent,
        "bet_divergence_rate": round(divergent as f64 / total.max(1) as f64),
    }))
}

fn parse_extension(payload: &str, vocab: &BTreeMap<String, BTreeSet<usize>>) -> Result<BTreeSet<usize>, String> {
    if let Some(rest) = payload.strip_prefix("ids:") {
        return Ok(rest
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse::<usize>())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("bad id: {e}"))?);
    }
    let (op, names) = payload
        .split_once(':')
        .ok_or_else(|| format!("reading `{payload}` must be ids:..., all_of:..., or any_of:..."))?;
    let names: Vec<&str> = names.split(',').filter(|n| !n.trim().is_empty()).collect();
    let mut sets = Vec::new();
    for name in &names {
        let set = vocab
            .get(name.trim())
            .ok_or_else(|| format!("unknown predicate `{name}` in a reading"))?;
        sets.push(set.clone());
    }
    match op {
        "all_of" => {
            let mut it = sets.into_iter();
            let mut acc = it.next().ok_or("all_of needs operands")?;
            for s in it {
                acc = acc.intersection(&s).copied().collect();
            }
            Ok(acc)
        }
        "any_of" => Ok(sets.into_iter().flatten().collect()),
        other => Err(format!("unknown reading kind `{other}`")),
    }
}

/// Complement coverage and axis equivalence rate over a readings table.
pub fn readings_report(vocab_path: &Path, readings_path: &Path) -> Result<serde_json::Value, String> {
    let mut vocab = BTreeMap::new();
    for row in read_table(vocab_path, &["name", "ids"])? {
        let ids: BTreeSet<usize> = row["ids"]
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse::<usize>())
            .collect::<Result<_, _>>()
            .map_err(|e| format!("bad id in vocabulary: {e}"))?;
        vocab.insert(row["name"].clone(), ids);
    }
    let rows = read_table(readings_path, &["name", "payload", "stated"])?;
    let mut stated = Vec::new();
    let mut other = Vec::new();
    for row in rows {
        let ext = parse_extension(&row["payload"], &vocab)?;
        let entry = (row["name"].clone(), ext);
        if row["stated"] == "yes" {
            stated.push(entry);
        } else {
            other.push(entry);
        }
    }
    let total = other.len();
    if total == 0 {
        return Ok(json!({"readings": 0}));
    }
    let mut equivalent = serde_json::Map::new();
    let mut complement = Vec::new();
    for (name, ext) in &other {
        match stated.iter().find(|(_, s)| s == ext) {
            Some((stated_name, _)) => {
                equivalent.insert(name.clone(), json!(stated_name));
            }
            None => complement.push(name.clone()),
        }
    }
    Ok(json!({
        "readings": total,
        "stated": stated.len(),
        "complement": complement.len(),
        "complement_coverage": round(complement.len() as f64 / total as f64),
        "axis_equivalence_rate": round(equivalent.len() as f64 / total as f64),
        "equivalent_to": equivalent,
        "complement_names": complement,
    }))
}

/// Turn one request/response/truth triple into the verdict table `verdict` consumes.
///
/// The response is a predicate over the object set, not a per-object verdict, so the rows are
/// derived: an object in the extension is `pos`, an object the candidate abstained on is `unknown`
/// (and takes priority over the extension), and everything else is `neg`. The candidate's score, if
/// it carries one, is copied to every row of that candidate -- a single number orders the candidate's
/// claims together and is not a per-object probability.
pub fn rows_report(
    request_path: &Path,
    response_path: &Path,
    truth_path: &Path,
    system: &str,
) -> Result<String, String> {
    let request: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(request_path)
            .map_err(|e| format!("cannot read {}: {e}", request_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", request_path.display()))?;
    let response: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(response_path)
            .map_err(|e| format!("cannot read {}: {e}", response_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", response_path.display()))?;
    let truth: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(truth_path)
            .map_err(|e| format!("cannot read {}: {e}", truth_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", truth_path.display()))?;

    let n = request["objects"]
        .as_u64()
        .ok_or("request has no `objects`")? as usize;
    let mut vocab: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for pred in request["vocabulary"].as_array().ok_or("request has no vocabulary")? {
        let name = pred["name"].as_str().ok_or("predicate without a name")?;
        let ids: BTreeSet<usize> = pred["ids"]
            .as_array()
            .ok_or("predicate without ids")?
            .iter()
            .filter_map(|v| v.as_u64().map(|x| x as usize))
            .collect();
        vocab.insert(name.to_string(), ids);
    }

    let candidate = response["candidates"]
        .as_array()
        .and_then(|c| c.first())
        .ok_or("response has no candidate")?;
    let extension: BTreeSet<usize> = if candidate["ids"].is_array() {
        candidate["ids"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64().map(|x| x as usize))
            .collect()
    } else {
        let define = candidate["define"].as_object().ok_or("candidate has neither ids nor define")?;
        let (op, names) = if let Some(ops) = define.get("all_of") {
            ("all_of", ops)
        } else {
            ("any_of", define.get("any_of").ok_or("define has no operator")?)
        };
        let names = names.as_array().ok_or("definition has no operands")?;
        let mut sets = Vec::new();
        for name in names {
            let name = name.as_str().ok_or("operand is not a name")?;
            sets.push(vocab.get(name).ok_or(format!("unknown predicate `{name}`"))?.clone());
        }
        match op {
            "all_of" => {
                let mut it = sets.into_iter();
                let mut acc = it.next().ok_or("all_of with no operands")?;
                for s in it {
                    acc = acc.intersection(&s).copied().collect();
                }
                acc
            }
            _ => sets.into_iter().flatten().collect(),
        }
    };
    let abstain: BTreeSet<usize> = candidate["abstain"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect())
        .unwrap_or_default();
    let score = candidate["score"].as_f64();

    let positives: BTreeSet<usize> = truth["positives"]
        .as_array()
        .ok_or("truth has no positives")?
        .iter()
        .filter_map(|v| v.as_u64().map(|x| x as usize))
        .collect();
    let negatives: BTreeSet<usize> = truth["negatives"]
        .as_array()
        .ok_or("truth has no negatives")?
        .iter()
        .filter_map(|v| v.as_u64().map(|x| x as usize))
        .collect();

    let score_cell = score.map(|s| s.to_string()).unwrap_or_default();
    let mut out = String::from("system\titem\tverdict\ttruth\tscore\n");
    for id in 0..n {
        let verdict = if abstain.contains(&id) {
            "unknown"
        } else if extension.contains(&id) {
            "pos"
        } else {
            "neg"
        };
        let actual = if positives.contains(&id) {
            "pos"
        } else if negatives.contains(&id) {
            "neg"
        } else {
            continue;
        };
        out.push_str(&format!("{system}\t{id}\t{verdict}\t{actual}\t{score_cell}\n"));
    }
    Ok(out.trim_end().to_string())
}

pub fn cmd(args: &[String]) -> i32 {
    let mut rest = args.iter().skip(1);
    let sub = match rest.next() {
        Some(s) => s.as_str(),
        None => {
            eprintln!("usage: metaform metrics verdict|bets|readings ...");
            return 2;
        }
    };
    let result = match sub {
        "verdict" => rest
            .next()
            .ok_or_else(|| "usage: metrics verdict <rows.tsv>".to_string())
            .and_then(|p| verdict_report(Path::new(p))),
        "bets" => rest
            .next()
            .ok_or_else(|| "usage: metrics bets <predictions.tsv>".to_string())
            .and_then(|p| bets_report(Path::new(p))),
        "readings" => {
            let args: Vec<&String> = rest.collect();
            let vocab = args
                .iter()
                .position(|a| a.as_str() == "--vocab")
                .and_then(|i| args.get(i + 1))
                .ok_or_else(|| "usage: metrics readings --vocab <tsv> --readings <tsv>".to_string());
            let readings = args
                .iter()
                .position(|a| a.as_str() == "--readings")
                .and_then(|i| args.get(i + 1))
                .ok_or_else(|| "usage: metrics readings --vocab <tsv> --readings <tsv>".to_string());
            vocab.and_then(|v| readings.map(|r| (v, r))).and_then(|(v, r)| {
                readings_report(Path::new(v), Path::new(r))
            })
        }
        "rows" => {
            let args: Vec<&String> = rest.collect();
            let value = |flag: &str| {
                args.iter()
                    .position(|a| a.as_str() == flag)
                    .and_then(|i| args.get(i + 1))
                    .map(|s| s.as_str())
            };
            match (value("--request"), value("--response"), value("--truth")) {
                (Some(r), Some(p), Some(t)) => {
                    let system = value("--system").unwrap_or("arm");
                    rows_report(Path::new(r), Path::new(p), Path::new(t), system)
                        .map(serde_json::Value::String)
                }
                _ => Err("usage: metrics rows --request <json> --response <json> --truth <json> [--system <name>]".to_string()),
            }
        }
        other => Err(format!("unknown metrics subcommand `{other}`")),
    };
    match result {
        Ok(value) => {
            match value {
                serde_json::Value::String(text) => println!("{text}"),
                other => println!("{other}"),
            }
            0
        }
        Err(message) => {
            eprintln!("{message}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("metrics-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.tsv");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path
    }

    #[test]
    fn wilson_brackets_the_point_estimate() {
        let (lo, hi) = wilson(9, 10);
        assert!(lo < 0.9 && hi > 0.9 && hi <= 1.0);
        let (lo, hi) = wilson(0, 5);
        assert_eq!(lo, 0.0);
        assert!(hi > 0.0);
    }

    #[test]
    fn verdict_report_separates_abstention_from_error() {
        let path = temp(
            "verdict",
            "system\titem\tverdict\ttruth\tscore\n\
             a\t1\tpos\tpos\t1\n\
             a\t2\tneg\tneg\t1\n\
             a\t3\tunknown\tneg\t0\n\
             b\t1\tpos\tneg\t1\n\
             b\t2\tpos\tneg\t1\n\
             b\t3\tpos\tpos\t1\n",
        );
        let report = verdict_report(&path).unwrap();
        assert_eq!(report["a"]["unknown_rate"], 0.333);
        assert_eq!(report["a"]["precision_on_answered"], 1.0);
        assert_eq!(report["b"]["unknown_rate"], 0.0);
        assert_eq!(report["b"]["precision_on_answered"], 0.333);
    }

    #[test]
    fn bets_without_a_rival_are_not_divergent() {
        let path = temp(
            "bets",
            "id\tpredicted\textension\trival\n\
             3\tpos\t3,4\t\n\
             4\tneg\t3,4\t4\n",
        );
        let report = bets_report(&path).unwrap();
        assert_eq!(report["bets"], 2);
        assert_eq!(report["divergent"], 0);
    }

    #[test]
    fn rows_turn_a_response_into_verdicts_with_abstention_first() {
        let dir = std::env::temp_dir().join(format!("metrics-rows-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let request = dir.join("request.json");
        let response = dir.join("response.json");
        let truth = dir.join("truth.json");
        std::fs::write(
            &request,
            r#"{"objects":4,"vocabulary":[{"name":"a","ids":[0,2]}],
                "evidence":{"pos":[0],"neg":[1]}}"#,
        )
        .unwrap();
        std::fs::write(
            &response,
            r#"{"candidates":[{"name":"c","define":{"any_of":["a"]},"abstain":[2],"score":0.5}]}"#,
        )
        .unwrap();
        std::fs::write(&truth, r#"{"positives":[0],"negatives":[1,2,3]}"#).unwrap();

        let text = rows_report(&request, &response, &truth, "arm").unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "system\titem\tverdict\ttruth\tscore");
        assert!(lines.contains(&"arm\t0\tpos\tpos\t0.5"), "{text}");
        assert!(lines.contains(&"arm\t1\tneg\tneg\t0.5"), "{text}");
        // The abstention wins over the definition, which would otherwise call object 2 positive.
        assert!(lines.contains(&"arm\t2\tunknown\tneg\t0.5"), "{text}");
        assert!(lines.contains(&"arm\t3\tneg\tneg\t0.5"), "{text}");
    }

    #[test]
    fn readings_separate_rewordings_from_new_angles() {
        let vocab = temp("vocab", "name\tids\na\t0,1\nb\t1,2\nc\t2,3\n");
        let readings = temp(
            "readings",
            "name\tpayload\tstated\n\
             a\tall_of:a,b\tyes\n\
             reword\tall_of:b,a\tno\n\
             wider\tany_of:a,c\tno\n",
        );
        let report = readings_report(&vocab, &readings).unwrap();
        assert_eq!(report["axis_equivalence_rate"], 0.5);
        assert_eq!(report["complement_coverage"], 0.5);
        assert_eq!(report["equivalent_to"]["reword"], "a");
    }
}
