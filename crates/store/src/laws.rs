//! `.metaform/laws.tsv`: the persisted theory `explore` starts from, plus recorded refutations.
//!
//! One tab-separated line per entry:
//!
//! ```text
//! source<TAB>kind<TAB>premise<TAB>conclusion<TAB>note
//! ```
//!
//! `kind` is `law` or `refutation`. A `law` is a Horn implication used by the closure; its `source`
//! is `evidence`, `expert` or `define`. A `refutation` records that the expert decided the premise
//! does *not* imply the conclusion, and its `note` names the counterexample object. Keeping both
//! kinds in one file means `explore` has exactly one theory path (`--laws`), and the `kind` column is
//! what stops a refutation from ever entering the closure as if it were a law: the loader separates
//! the two before `Context::run_with_theory_and_refutations` sees them.
//!
//! Premise and conclusion are comma-separated predicate names; the note is free text without tabs.
//! Names are resolved back to the context's bitmasks on load, so the file survives a change in the
//! state's predicate order.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use metaform_kernel::canonical_names;

/// The required first line of a non-empty theory file.
pub const HEADER: &str = "source\tkind\tpremise\tconclusion\tnote";

/// Accepted provenances of an entry.
pub const SOURCES: [&str; 3] = ["evidence", "expert", "define"];

/// The two entry kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Law,
    Refutation,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Law => "law",
            Kind::Refutation => "refutation",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "law" => Some(Kind::Law),
            "refutation" => Some(Kind::Refutation),
            _ => None,
        }
    }
}

/// One persisted entry. `premise`/`conclusion` are canonicalised to sorted, unique order so
/// comparisons and dedupe are order-independent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: String,
    pub kind: Kind,
    pub premise: Vec<String>,
    pub conclusion: Vec<String>,
    pub note: String,
}

impl Entry {
    pub fn law(
        source: impl Into<String>,
        premise: Vec<String>,
        conclusion: Vec<String>,
        note: impl Into<String>,
    ) -> Self {
        Self::new(source, Kind::Law, premise, conclusion, note)
    }

    pub fn refutation(
        source: impl Into<String>,
        premise: Vec<String>,
        conclusion: Vec<String>,
        note: impl Into<String>,
    ) -> Self {
        Self::new(source, Kind::Refutation, premise, conclusion, note)
    }

    fn new(
        source: impl Into<String>,
        kind: Kind,
        premise: Vec<String>,
        conclusion: Vec<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            kind,
            premise: canonical_names(premise),
            conclusion: canonical_names(conclusion),
            note: note.into(),
        }
    }

    /// Dedupe identity: kind + source + premise + conclusion. The note is not part of it: replaying
    /// the same answer must not append a second line just because the note text changed.
    fn key(&self) -> (String, String, Vec<String>, Vec<String>) {
        (
            self.kind.as_str().to_string(),
            self.source.clone(),
            canonical_names(self.premise.clone()),
            canonical_names(self.conclusion.clone()),
        )
    }
}

/// A refutation with masks resolved against the context. `counterexample` is read back from the
/// note when it names an object, so a hand-written entry without one still loads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaskedRefutation {
    pub premise: u64,
    pub conclusion: u64,
    pub counterexample: Option<usize>,
    pub source: String,
}

/// Read and parse a theory file. A missing (or empty) file is the empty theory, not an error.
pub fn load(path: &Path) -> Result<Vec<Entry>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read `{}`: {e}", path.display())),
    };
    parse(&text).map_err(|e| format!("cannot parse `{}`: {e}", path.display()))
}

/// Split a parsed file into the Horn theory and the recorded refutations, resolving every name to
/// the context's predicate bits. An unknown name is an error that names it; it is never silently
/// dropped, because a missing entry would silently change the questions.
pub fn masks(
    entries: &[Entry],
    names: &[String],
) -> Result<(Vec<(u64, u64)>, Vec<MaskedRefutation>), String> {
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, name) in names.iter().enumerate() {
        index.entry(name.as_str()).or_insert(i);
    }
    let mut laws = Vec::new();
    let mut refutations = Vec::new();
    for entry in entries {
        let premise = mask_of(&entry.premise, &index).map_err(|e| {
            format!(
                "{e} in {} `{} -> {}`",
                entry.kind.as_str(),
                entry.premise.join(","),
                entry.conclusion.join(",")
            )
        })?;
        let conclusion = mask_of(&entry.conclusion, &index).map_err(|e| {
            format!(
                "{e} in {} `{} -> {}`",
                entry.kind.as_str(),
                entry.premise.join(","),
                entry.conclusion.join(",")
            )
        })?;
        match entry.kind {
            Kind::Law => laws.push((premise, conclusion)),
            Kind::Refutation => refutations.push(MaskedRefutation {
                premise,
                conclusion,
                counterexample: counterexample_of(&entry.note),
                source: entry.source.clone(),
            }),
        }
    }
    Ok((laws, refutations))
}

/// What an [`append`] wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appended {
    pub laws: usize,
    pub refutations: usize,
    pub duplicates: usize,
}

/// Append entries to `path`, creating the file and its header when needed.
///
/// Rows dedupe on kind + source + premise + conclusion against both the existing file and the batch
/// itself, so replaying the same answers file is a no-op. Returns per-kind counts and how many rows
/// were already present.
pub fn append(path: &Path, entries: &[Entry]) -> Result<Appended, String> {
    for entry in entries {
        if !SOURCES.contains(&entry.source.as_str()) {
            return Err(format!("unknown source `{}`", entry.source));
        }
        if entry.kind == Kind::Refutation {
            if !matches!(entry.source.as_str(), "evidence" | "expert") {
                return Err(format!(
                    "a refutation needs source `evidence` or `expert`, got `{}`",
                    entry.source
                ));
            }
            if entry.conclusion.is_empty() {
                return Err("a refutation needs a conclusion it does not imply".into());
            }
        }
        for name in entry.premise.iter().chain(entry.conclusion.iter()) {
            if name.is_empty() || name.contains([',', '\t', '\n', '\r']) {
                return Err(format!("predicate name `{name}` cannot be written to a laws file"));
            }
        }
    }

    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read `{}`: {e}", path.display())),
    };
    let existing_entries =
        parse(&existing).map_err(|e| format!("cannot parse `{}`: {e}", path.display()))?;
    let mut keys: HashSet<(String, String, Vec<String>, Vec<String>)> =
        existing_entries.iter().map(Entry::key).collect();

    let mut appended = Appended {
        laws: 0,
        refutations: 0,
        duplicates: 0,
    };
    let mut body = String::new();
    for entry in entries {
        if !keys.insert(entry.key()) {
            appended.duplicates += 1;
            continue;
        }
        body.push_str(&line(entry));
        match entry.kind {
            Kind::Law => appended.laws += 1,
            Kind::Refutation => appended.refutations += 1,
        }
    }
    if appended.laws + appended.refutations == 0 {
        return Ok(appended);
    }

    let mut text = existing;
    if text.trim().is_empty() {
        text = format!("{HEADER}\n");
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&body);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create `{}`: {e}", parent.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
    Ok(appended)
}

/// Parse the file body. Empty input is the empty theory; any other content must start with
/// [`HEADER`] and every following non-blank line must have four or five fields.
fn parse(text: &str) -> Result<Vec<Entry>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines().enumerate();
    let (header_no, header) = match lines.next() {
        Some((i, line)) => (i + 1, line.trim_end_matches('\r')),
        None => return Ok(Vec::new()),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    if header.trim() != HEADER {
        return Err(format!(
            "line {header_no}: expected the header `{HEADER}`, got `{header}`"
        ));
    }

    let mut out = Vec::new();
    for (i, raw) in lines {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let no = i + 1;
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 4 || fields.len() > 5 {
            return Err(format!(
                "line {no}: expected 4 or 5 tab-separated fields, got {}",
                fields.len()
            ));
        }
        let source = fields[0].trim().to_string();
        if !SOURCES.contains(&source.as_str()) {
            return Err(format!(
                "line {no}: unknown source `{source}` (expected evidence, expert or define)"
            ));
        }
        let kind = Kind::parse(fields[1].trim()).ok_or_else(|| {
            format!(
                "line {no}: unknown kind `{}` (expected law or refutation)",
                fields[1].trim()
            )
        })?;
        if kind == Kind::Refutation && !matches!(source.as_str(), "evidence" | "expert") {
            return Err(format!(
                "line {no}: a refutation needs source `evidence` or `expert`, got `{source}`"
            ));
        }
        let premise = parse_names(fields[2], no, "premise")?;
        let conclusion = parse_names(fields[3], no, "conclusion")?;
        let note = fields
            .get(4)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if note.contains('\t') {
            return Err(format!("line {no}: the note must not contain a tab"));
        }
        if kind == Kind::Refutation && conclusion.is_empty() {
            return Err(format!(
                "line {no}: a refutation needs a conclusion it does not imply"
            ));
        }
        out.push(Entry {
            source,
            kind,
            premise,
            conclusion,
            note,
        });
    }
    Ok(out)
}

fn parse_names(raw: &str, line: usize, what: &str) -> Result<Vec<String>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for part in raw.split(',') {
        let name = part.trim();
        if name.is_empty() {
            return Err(format!("line {line}: empty {what} predicate name"));
        }
        names.push(name.to_string());
    }
    Ok(canonical_names(names))
}

fn mask_of(names: &[String], index: &HashMap<&str, usize>) -> Result<u64, String> {
    let mut mask = 0u64;
    for name in names {
        let Some(&i) = index.get(name.as_str()) else {
            return Err(format!("unknown predicate `{name}`"));
        };
        mask |= 1u64 << i;
    }
    Ok(mask)
}

/// Read the object id out of a canonical note such as `refuted by object 3: lacks a`. A hand-written
/// note that does not name an object still loads; the report then omits the `counterexample` field.
fn counterexample_of(note: &str) -> Option<usize> {
    let words: Vec<&str> = note.split_whitespace().collect();
    for pair in words.windows(2) {
        if pair[0].eq_ignore_ascii_case("object") {
            let digits: String = pair[1].chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                if let Ok(id) = digits.parse::<usize>() {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// One serialized entry. Tabs/newlines in the note would break the column layout, so they become
/// spaces.
fn line(entry: &Entry) -> String {
    let note: String = entry
        .note
        .chars()
        .map(|c| if c == '\t' || c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        entry.source,
        entry.kind.as_str(),
        canonical_names(entry.premise.clone()).join(","),
        canonical_names(entry.conclusion.clone()).join(","),
        note.trim()
    )
}
