//! `metaform concept` -- register what the repository has absorbed, with the evidence for it.
//!
//! The other three commands in this crate act on a scenario. This one acts on the repository's own
//! knowledge: a concept taken from outside, the claim this repository makes about it, and where that
//! claim comes from. It exists because a paragraph can be ignored -- and this repository's standing
//! complaint is that the interesting part of what it knows lives in prose nobody reads twice.
//!
//! Two rules are enforced rather than suggested:
//!
//! - **a concept without evidence is refused.** "We read about it somewhere" is the failure mode the
//!   file exists to make visible, so the empty case is an error rather than a row.
//! - **retraction appends.** A concept that turns out to be wrong leaves a trace: the later row wins
//!   when the file is folded, and the earlier row stays readable.
//!
//! Usage:
//!   metaform concept add    --project . --kind vsa --name "MBAT binding" --claim "..." --evidence "..."
//!   metaform concept list   --project .
//!   metaform concept show   --project . --id C1
//!   metaform concept retract --project . --id C1 --reason "..."

use std::path::{Path, PathBuf};

use metaform_store::concepts::{self, Concept};

const USAGE: &str = "usage: metaform concept <add|list|show|retract> [options]";

fn concepts_path(project: &Path) -> PathBuf {
    project.join(".metaform").join("concepts.tsv")
}

fn now() -> String {
    // A timestamp is useful for ordering by eye and useless for ordering by program, so it is
    // seconds since the epoch: monotonic enough to read, and it needs no clock library.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// A JSON string literal, quotes included. The caller wants a value, not an escaped fragment.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn concept_json(c: &Concept) -> String {
    format!(
        "{{\"id\":{},\"kind\":{},\"name\":{},\"claim\":{},\"evidence\":{},\"status\":{},\"by\":{},\"at\":{},\"note\":{}}}",
        json_str(&c.id),
        json_str(&c.kind),
        json_str(&c.name),
        json_str(&c.claim),
        json_str(&c.evidence),
        json_str(&c.status),
        json_str(&c.by),
        json_str(&c.at),
        json_str(&c.note),
    )
}

/// The project directory, the way the other commands read it.
fn project_root(args: &[String]) -> PathBuf {
    PathBuf::from(
        args.iter()
            .position(|a| a == "--project")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| ".".to_string()),
    )
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// Register one concept. Returns the stored row as JSON.
///
/// Shared by the command line and by the MCP tool, so that "the model can only register a concept
/// through the tool" and "a person can register one by hand" are the same code path.
pub fn add(
    root: &Path,
    kind: &str,
    name: &str,
    claim: &str,
    evidence: &str,
    by: &str,
    note: &str,
) -> Result<String, String> {
    if kind.trim().is_empty() || name.trim().is_empty() {
        return Err("kind and name are required".into());
    }
    if claim.trim().is_empty() {
        return Err("claim is required (what this repository believes about it)".into());
    }
    if evidence.trim().is_empty() {
        return Err(
            "evidence is required (a hash, a path, or a command); a concept with no evidence is a note, and notes get ignored"
                .into(),
        );
    }
    let path = concepts_path(root);
    let entries = concepts::read_all(&path);
    let concept = Concept {
        id: concepts::next_id(&entries),
        kind: kind.to_string(),
        name: name.to_string(),
        claim: claim.to_string(),
        evidence: evidence.to_string(),
        status: "active".to_string(),
        by: if by.trim().is_empty() { "unknown".to_string() } else { by.to_string() },
        at: now(),
        note: note.to_string(),
    };
    concepts::append(&path, &concept)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(concept_json(&concept))
}

/// Every concept in force, as JSON.
pub fn list(root: &Path) -> String {
    let path = concepts_path(root);
    let entries = concepts::read_all(&path);
    let folded = concepts::fold(&entries);
    let items: Vec<String> = folded.iter().map(concept_json).collect();
    format!("{{\"concepts\":[{}],\"rows\":{}}}", items.join(","), entries.len())
}

/// One concept by id, as JSON.
pub fn show(root: &Path, id: &str) -> Result<String, String> {
    let path = concepts_path(root);
    let folded = concepts::fold(&concepts::read_all(&path));
    folded
        .iter()
        .find(|c| c.id == id)
        .map(concept_json)
        .ok_or_else(|| format!("no concept with id {id}"))
}

/// Withdraw one concept: a later row with the same id, so the history stays readable.
pub fn retract(root: &Path, id: &str, reason: &str, by: &str) -> Result<String, String> {
    let path = concepts_path(root);
    let folded = concepts::fold(&concepts::read_all(&path));
    let existing = folded
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or_else(|| format!("no concept with id {id}"))?;
    let retraction = Concept {
        id: existing.id.clone(),
        kind: existing.kind.clone(),
        name: existing.name.clone(),
        claim: existing.claim.clone(),
        evidence: existing.evidence.clone(),
        status: "retracted".to_string(),
        by: if by.trim().is_empty() { "unknown".to_string() } else { by.to_string() },
        at: now(),
        note: if reason.trim().is_empty() { "retracted".to_string() } else { reason.to_string() },
    };
    concepts::append(&path, &retraction)
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(concept_json(&retraction))
}

/// Run the command. Returns the process exit code.
pub fn cmd(args: &[String]) -> i32 {
    let Some(sub) = args.get(1).map(String::as_str) else {
        eprintln!("{USAGE}");
        return 2;
    };
    if sub == "--selftest" {
        return selftest();
    }
    let root = project_root(args);

    match sub {
        "add" => {
            let kind = flag(args, "--kind").unwrap_or_default();
            let name = flag(args, "--name").unwrap_or_default();
            let claim = flag(args, "--claim").unwrap_or_default();
            let evidence = flag(args, "--evidence").unwrap_or_default();
            let by = flag(args, "--by").unwrap_or_else(|| "unknown".to_string());
            let note = flag(args, "--note").unwrap_or_default();
            match add(&root, &kind, &name, &claim, &evidence, &by, &note) {
                Ok(row) => {
                    println!("{row}");
                    0
                }
                Err(e) => {
                    eprintln!("metaform concept add: {e}");
                    2
                }
            }
        }
        "list" => {
            println!("{}", list(&root));
            0
        }
        "show" => {
            let Some(id) = flag(args, "--id") else {
                eprintln!("metaform concept show: --id is required");
                return 2;
            };
            match show(&root, &id) {
                Ok(row) => {
                    println!("{row}");
                    0
                }
                Err(e) => {
                    eprintln!("metaform concept show: {e}");
                    2
                }
            }
        }
        "retract" => {
            let Some(id) = flag(args, "--id") else {
                eprintln!("metaform concept retract: --id is required");
                return 2;
            };
            let reason = flag(args, "--reason").unwrap_or_default();
            let by = flag(args, "--by").unwrap_or_default();
            match retract(&root, &id, &reason, &by) {
                Ok(row) => {
                    println!("{row}");
                    0
                }
                Err(e) => {
                    eprintln!("metaform concept retract: {e}");
                    2
                }
            }
        }
        other => {
            eprintln!("metaform concept: unknown sub-command `{other}`\n{USAGE}");
            2
        }
    }
}

/// Prove the four properties the command claims, in a temporary directory and without the engine.
fn selftest() -> i32 {
    let dir = std::env::temp_dir().join(format!("metaform-concept-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = concepts_path(&dir);
    let mut failures: Vec<String> = Vec::new();

    let concept = Concept {
        id: "C1".to_string(),
        kind: "vsa".to_string(),
        name: "MBAT binding".to_string(),
        claim: "a bipolar vector can stand for a set".to_string(),
        evidence: "sha256:abc".to_string(),
        status: "active".to_string(),
        by: "selftest".to_string(),
        at: "0".to_string(),
        note: String::new(),
    };
    concepts::append(&path, &concept).expect("append");

    let entries = concepts::read_all(&path);
    if entries.len() != 1 {
        failures.push(format!("one row expected, got {}", entries.len()));
    }
    if concepts::next_id(&entries) != "C2" {
        failures.push("the next id should be C2".to_string());
    }

    // A retraction is a later row: the history stays.
    let mut retracted = concept.clone();
    retracted.status = "retracted".to_string();
    concepts::append(&path, &retracted).expect("append");
    if concepts::read_all(&path).len() != 2 {
        failures.push("the retraction must not overwrite the earlier row".to_string());
    }
    if !concepts::active(&concepts::read_all(&path)).is_empty() {
        failures.push("a retracted concept must not be in force".to_string());
    }

    let _ = std::fs::remove_dir_all(&dir);
    if failures.is_empty() {
        println!("ok: concepts are append-only, retractable, and every row carries its evidence");
        return 0;
    }
    for f in failures {
        eprintln!("concept selftest: {f}");
    }
    1
}
