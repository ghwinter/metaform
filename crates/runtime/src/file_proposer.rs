//! A proposer backed by a file.
//!
//! This is how anything that is not a Rust component -- a language model, a script, a person with a
//! text editor -- puts candidates on the table. Each line names the state it belongs to, a predicate
//! name, and the object ids that satisfy it:
//!
//! ```text
//! # state          name              ids
//! k2_0              runs_a_process    5 10 14 20 22 24 27 28
//! ```
//!
//! The state is matched by the label the caller gave it (the `request_id` in a state file). A
//! proposal that arrives for another state is ignored rather than applied to the wrong one, and an
//! empty or missing file simply offers nothing.

use crate::{Candidate, Context, OpStatus, Operator, OperatorResult};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct FileProposer {
    /// Label -> proposals.
    proposals: BTreeMap<String, Vec<Candidate>>,
    /// How many lines were read, for reporting.
    pub lines: usize,
}

impl FileProposer {
    pub fn load(path: &Path) -> std::io::Result<FileProposer> {
        let mut proposer = FileProposer::default();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(proposer),
            Err(e) => return Err(e),
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 2 {
                continue;
            }
            let label = fields[0].to_string();
            let name = fields[1].to_string();
            let mut extension: u64 = 0;
            for id in &fields[2..] {
                if let Ok(i) = id.parse::<usize>() {
                    if i < 64 {
                        extension |= 1u64 << i;
                    }
                }
            }
            proposer
                .proposals
                .entry(label)
                .or_default()
                .push(Candidate {
                    name,
                    extension,
                    source: "file".to_string(),
                    detail: "supplied in a proposals file".to_string(),
                    warrant: vec!["declared by the file's author".to_string()],
                    falsifier: vec!["a labelled object that contradicts the extension".to_string()],
                    adoptable: true,
                });
            proposer.lines += 1;
        }
        Ok(proposer)
    }
}

impl Operator for FileProposer {
    fn name(&self) -> &'static str {
        "file-proposer"
    }

    fn applicable(&self, ctx: &Context) -> bool {
        self.proposals.contains_key(&ctx.label)
    }

    fn estimate_cost(&self, _ctx: &Context) -> f32 {
        // A file lookup; the cost of producing the proposals is the author's, not the runtime's.
        0.0
    }

    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult {
        match self.proposals.get(&ctx.label) {
            None => OperatorResult::not_applicable("no proposals for this state"),
            Some(candidates) => OperatorResult {
                candidates: candidates.iter().take(budget).cloned().collect(),
                evidence: vec![format!("{} proposal(s) for {}", candidates.len(), ctx.label)],
                assumptions: Vec::new(),
                unresolved: Vec::new(),
                cost: 0.0,
                status: OpStatus::Applied,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;

    fn write(text: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("metaform-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("proposals-{}.txt", COUNTER.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn proposals_are_matched_by_label() {
        let path = write("# comment\nk2_0 a&b 0 1\nk3_1 c 2\n");
        let proposer = FileProposer::load(&path).expect("load");
        assert_eq!(proposer.lines, 2);

        let mut ctx = context(4, Default::default(), Vec::new());
        ctx.label = "k2_0".to_string();
        assert!(proposer.applicable(&ctx));
        let result = proposer.execute(&ctx, 4);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].extension, 0b11);

        let mut other = context(4, Default::default(), Vec::new());
        other.label = "unseen".to_string();
        assert!(!proposer.applicable(&other));
    }

    #[test]
    fn a_missing_file_offers_nothing() {
        let path = std::env::temp_dir().join("metaform-file-missing.txt");
        let _ = std::fs::remove_file(&path);
        let proposer = FileProposer::load(&path).expect("no error");
        assert_eq!(proposer.lines, 0);
        let mut ctx = context(4, Default::default(), Vec::new());
        ctx.label = "any".to_string();
        assert!(!proposer.applicable(&ctx));
    }
}
