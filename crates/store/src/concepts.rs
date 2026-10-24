//! Concepts: what the repository has taken in, and what it believes about it.
//!
//! The ledger records evidence about *objects*. This records what the repository has *absorbed*: a
//! concept taken from outside (a vector-symbolic encoding, an induction procedure, a structure-mapping
//! algorithm), a claim about what it does here, and the evidence that the claim rests on.
//!
//! Why it is a file and not a paragraph: a paragraph can be ignored, and this repository has a
//! standing complaint that the interesting part of its knowledge lives in prose nobody reads twice. A
//! row here is append-only, carries a status, and can be retracted in place, so a concept that turns
//! out to be wrong leaves a trace instead of quietly disappearing.
//!
//! The schema is deliberately narrow: a concept without evidence is refused by the caller, because
//! "we read about it somewhere" is the failure mode this file exists to make visible.

use std::path::Path;

pub const HEADER: &str =
    "# metaform concepts v1\nid\tkind\tname\tclaim\tevidence\tstatus\tby\tat\tnote\n";

/// One registered concept. `status` is `active` or `retracted`; a retraction is a later row with the
/// same id, never an edit of the earlier one.
#[derive(Clone, Debug, PartialEq)]
pub struct Concept {
    pub id: String,
    /// Which family the concept came from: `vsa`, `ilp`, `sme`, `axis`, or anything else.
    pub kind: String,
    pub name: String,
    /// One sentence: what this repository believes about it.
    pub claim: String,
    /// Where the belief comes from: a hash, a path, a command. Never empty.
    pub evidence: String,
    pub status: String,
    pub by: String,
    pub at: String,
    pub note: String,
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Every row in the file, in the order it was written.
pub fn read_all(path: &Path) -> Vec<Concept> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        // The column-name row is not data; it would otherwise parse as a concept named "name".
        .filter(|l| !l.starts_with("id\t"))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 9 {
                return None;
            }
            Some(Concept {
                id: f[0].to_string(),
                kind: f[1].to_string(),
                name: f[2].to_string(),
                claim: f[3].to_string(),
                evidence: f[4].to_string(),
                status: f[5].to_string(),
                by: f[6].to_string(),
                at: f[7].to_string(),
                note: f[8].to_string(),
            })
        })
        .collect()
}

/// Fold by id: a later row replaces the earlier one, keeping the position where the id first appeared.
pub fn fold(entries: &[Concept]) -> Vec<Concept> {
    let mut out: Vec<Concept> = Vec::new();
    for e in entries {
        match out.iter_mut().find(|x| x.id == e.id) {
            Some(slot) => *slot = e.clone(),
            None => out.push(e.clone()),
        }
    }
    out
}

/// The concepts that are still in force.
pub fn active(entries: &[Concept]) -> Vec<Concept> {
    fold(entries)
        .into_iter()
        .filter(|c| c.status != "retracted")
        .collect()
}

/// The next free identifier: `C1`, `C2`, ...
pub fn next_id(entries: &[Concept]) -> String {
    let max = entries
        .iter()
        .filter_map(|c| c.id.strip_prefix('C').and_then(|n| n.parse::<usize>().ok()))
        .max()
        .unwrap_or(0);
    format!("C{}", max + 1)
}

/// Append one row. The file is never rewritten: a retraction is a new row with the same id.
pub fn append(path: &Path, c: &Concept) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    if text.is_empty() {
        text.push_str(HEADER);
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        clean(&c.id),
        clean(&c.kind),
        clean(&c.name),
        clean(&c.claim),
        clean(&c.evidence),
        clean(&c.status),
        clean(&c.by),
        clean(&c.at),
        clean(&c.note),
    ));
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concept(id: &str, status: &str) -> Concept {
        Concept {
            id: id.to_string(),
            kind: "vsa".to_string(),
            name: "MBAT binding".to_string(),
            claim: "a bipolar vector can stand for a set".to_string(),
            evidence: "sha256:abc".to_string(),
            status: status.to_string(),
            by: "test".to_string(),
            at: "2026-10-24".to_string(),
            note: String::new(),
        }
    }

    fn temp_file(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("metaform-concepts-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("concepts.tsv")
    }

    #[test]
    fn a_row_is_read_back_and_folded() {
        let path = temp_file("read");
        append(&path, &concept("C1", "active")).expect("append");
        let rows = read_all(&path);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "vsa");
        assert_eq!(rows[0].evidence, "sha256:abc");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_retraction_is_a_later_row_not_an_edit() {
        let path = temp_file("retract");
        append(&path, &concept("C1", "active")).expect("append");
        append(&path, &concept("C1", "retracted")).expect("append");
        // The file keeps both rows: the history is the point.
        assert_eq!(read_all(&path).len(), 2);
        // The fold shows one concept, and it is retracted.
        let folded = fold(&read_all(&path));
        assert_eq!(folded.len(), 1);
        assert_eq!(folded[0].status, "retracted");
        assert!(active(&read_all(&path)).is_empty(), "a retracted concept is not in force");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_next_identifier_follows_the_highest_one() {
        let rows = vec![concept("C1", "active"), concept("C7", "active")];
        assert_eq!(next_id(&rows), "C8");
    }

    #[test]
    fn a_tab_inside_a_field_becomes_a_space() {
        let path = temp_file("clean");
        let mut c = concept("C1", "active");
        c.claim = "one\ttwo\nthree".to_string();
        append(&path, &c).expect("append");
        let rows = read_all(&path);
        assert_eq!(rows[0].claim, "one two three");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
