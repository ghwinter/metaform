//! The kernel: the objects the measure is defined on.
//!
//! A vocabulary, the equivalence classes it induces, and the status those classes license. Nothing
//! here knows about a protocol, a file, a command line or a server -- which is the point of it being a
//! crate rather than a module: `cargo` refuses an import that points upwards, so the layering is
//! enforced by the compiler instead of by a scan someone has to keep in step.
//!
//! `metaform` re-exports every name below, so a caller keeps writing `metaform::Mask`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Bitmask over a set of objects. Enough for up to 64 objects; switch to `bitvec` if you need more.
pub type Mask = u64;

/// Whether two paths name the same file. Used so `--out` cannot overwrite the state that the
/// read-only command just loaded. A path that does not exist yet is compared by its canonical
/// parent plus file name.
pub fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    fn normalize(p: &std::path::Path) -> std::path::PathBuf {
        if let Ok(canonical) = p.canonicalize() {
            return canonical;
        }
        let parent = p.parent().unwrap_or_else(|| std::path::Path::new("."));
        let parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
        parent.join(p.file_name().unwrap_or_default())
    }
    normalize(a) == normalize(b)
}

/// The bit of the `i`-th object.
pub fn bit(i: usize) -> Mask {
    1 << i
}

/// The Boolean operator of a stored definition. A definition is a combination of predicates that
/// already exist in the vocabulary, not a new atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefOp {
    AllOf,
    AnyOf,
}

/// Sorted, deduplicated names: the canonical spelling of a definition's operands and of a law row.
/// Every writer emits this order, so comparisons and dedupe are order-independent.
pub fn canonical_names(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// A stored predicate definition: `all_of` is intersection, `any_of` is union of its operands.
///
/// The definition is kept next to the extension so the extension can be recomputed when the object
/// set changes; the persistence layer in the binary keeps it in `.metaform/state.json`.
///
/// Operands are commutative, so their order carries no meaning. Construction through
/// [`Definition::new`] and comparison through the manual `PartialEq` both use
/// [`canonical_names`], making `all_of:[b,a]` and `all_of:[a,b]` the same definition even when a
/// caller builds the struct literally.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Definition {
    pub op: DefOp,
    pub operands: Vec<String>,
}

impl Definition {
    /// Build a definition with canonical (sorted, unique) operands.
    pub fn new(op: DefOp, operands: Vec<String>) -> Self {
        Self {
            op,
            operands: canonical_names(operands),
        }
    }

    /// Rewrite the operands in canonical order. A path that must keep the raw list long enough to
    /// reject a duplicate operand calls this after that validation, not before it.
    pub fn canonicalize(&mut self) {
        self.operands = canonical_names(std::mem::take(&mut self.operands));
    }
}

/// Order-insensitive equality on the operands: the same Boolean definition compares equal however
/// its operands were spelled or ordered.
impl PartialEq for Definition {
    fn eq(&self, other: &Self) -> bool {
        self.op == other.op
            && canonical_names(self.operands.clone()) == canonical_names(other.operands.clone())
    }
}

impl Eq for Definition {}

/// A predicate: name + extension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Predicate {
    pub name: String,
    pub ext: Mask,
    /// How the extension was defined, when it was adopted from a definition rather than listed.
    /// The extension is still the authoritative value inside one fixed object set.
    pub definition: Option<Definition>,
}

impl Predicate {
    pub fn new(name: impl Into<String>, ext: Mask) -> Self {
        Self { name: name.into(), ext, definition: None }
    }

    /// A predicate adopted from a definition over existing predicates.
    pub fn defined(name: impl Into<String>, ext: Mask, mut definition: Definition) -> Self {
        definition.canonicalize();
        Self { name: name.into(), ext, definition: Some(definition) }
    }
}

/// Vocabulary: a small set of predicates. The order fixes the coordinate order of signatures.
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    pub preds: Vec<Predicate>,
}

impl Vocabulary {
    pub fn with(preds: Vec<Predicate>) -> Self {
        Self { preds }
    }

    pub fn len(&self) -> usize {
        self.preds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.preds.is_empty()
    }

    pub fn add(&mut self, p: Predicate) {
        self.preds.push(p);
    }
}

/// Exploration status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// A positive and a negative example fall in the same equivalence class: the current vocabulary is proven insufficient (T2).
    Insufficient,
    /// Every equivalence class is covered by a label: the target's extension is fully determined.
    Identified,
    /// Some classes are still uncovered; keep asking.
    Undetermined,
}

/// One exploration: a fixed object domain, a maintained vocabulary, and labeled samples.
#[derive(Clone, Debug)]
pub struct Explorer {
    pub n: usize,
    pub vocab: Vocabulary,
    pub pos: Mask,
    pub neg: Mask,
    /// Vote counts `(yes, no)` per object. Under a noisy oracle, repeated questions aggregate here.
    pub votes: Vec<(u8, u8)>,
    /// Objects that were asked directly. A class label 'inherits' to members never asked, so this records which ones were really asked.
    pub asked: Mask,
}

impl Explorer {
    pub fn new(n: usize, vocab: Vocabulary) -> Self {
        assert!(n <= 64, "Mask is u64, so at most 64 objects");
        Self { n, vocab, pos: 0, neg: 0, votes: vec![(0, 0); n], asked: 0 }
    }

    /// Signature of `x` under the current vocabulary.
    pub fn signature(&self, x: usize) -> Vec<bool> {
        let b = bit(x);
        self.vocab.preds.iter().map(|p| p.ext & b != 0).collect()
    }

    /// Group by signature and return the object mask of each equivalence class.
    pub fn classes(&self) -> Vec<Mask> {
        let mut groups: HashMap<Vec<bool>, Mask> = HashMap::new();
        for x in 0..self.n {
            *groups.entry(self.signature(x)).or_insert(0) |= bit(x);
        }
        groups.into_values().collect()
    }

    /// Current status (vote-margin threshold 1, i.e. 'a majority is enough').
    pub fn status(&self) -> Status {
        self.status_with(1)
    }

    /// Status with an explicit vote-margin threshold.
    ///
    /// `theta` is the minimum vote margin needed for 'confirmation': only objects with `yes - no >= theta` count as strong positives,
    /// and `no - yes >= theta` as strong negatives. The more noise, the larger `theta` should be (see RESULTS §3.11:
    /// in the full loop, at ε=0.1 the θ=1 setting succeeds only 25% of the time, while θ=3 reaches 98%).
    ///
    /// Three rules (aligned with `experiments/noisy_loop.py`):
    /// 1. Collision first: a class has both a strong positive and a strong negative → `Insufficient`.
    /// 2. Weak votes block: a class still holds a member that 'voted but did not reach θ' → not decided, return `Undetermined`.
    ///    Otherwise 'one strong label + one weak objection' would be silently accepted, yielding a wrong extension.
    /// 3. Cover: every class needs a strong label, otherwise `Undetermined`.
    pub fn status_with(&self, theta: u8) -> Status {
        let (sp, sn) = self.strong(theta);
        let classes = self.classes();
        if classes.iter().any(|c| c & sp != 0 && c & sn != 0) {
            return Status::Insufficient;
        }
        let weak = self.voted_mask() & !(sp | sn);
        if classes.iter().any(|c| c & weak != 0) {
            return Status::Undetermined;
        }
        let known = sp | sn;
        if classes.iter().any(|c| c & known == 0) {
            return Status::Undetermined;
        }
        Status::Identified
    }

    /// Objects that have voted (strong or weak).
    pub fn voted_mask(&self) -> Mask {
        self.votes
            .iter()
            .enumerate()
            .filter(|(_, &(y, n))| y + n > 0)
            .fold(0u64, |acc, (x, _)| acc | bit(x))
    }

    /// Objects that need more votes: they voted, but are not strong labels under threshold `theta`.
    pub fn pending(&self, theta: u8) -> Vec<usize> {
        let (sp, sn) = self.strong(theta);
        let weak = self.voted_mask() & !(sp | sn);
        (0..self.n).filter(|&x| weak & bit(x) != 0).collect()
    }

    /// Estimate the oracle's error rate directly from the votes: the minority-vote share of each object, averaged over the objects that voted.
    ///
    /// With few votes this estimate is biased low (at r=1 it can only be 0 or 1), so it suits use 'after a few rounds',
    /// not the first round.
    pub fn implied_epsilon(&self) -> f64 {
        let mut sum = 0.0;
        let mut n = 0u32;
        for &(yes, no) in &self.votes {
            let total = yes as u32 + no as u32;
            if total > 0 {
                sum += (yes.min(no) as f64) / (total as f64);
                n += 1;
            }
        }
        if n == 0 {
            0.0
        } else {
            sum / n as f64
        }
    }

    /// Pick a θ from the estimated ε (empirical rule of `RESULTS.md` §3.11).
    pub fn recommended_theta(&self) -> u8 {
        let e = self.implied_epsilon();
        if e <= 0.10 {
            3
        } else if e >= 0.20 {
            5
        } else {
            4
        }
    }

    /// Masks of the 'strong positives / strong negatives' under the current threshold.
    pub fn strong(&self, theta: u8) -> (Mask, Mask) {
        let mut sp = 0u64;
        let mut sn = 0u64;
        for (x, &(yes, no)) in self.votes.iter().enumerate() {
            if yes > no && yes - no >= theta {
                sp |= bit(x);
            } else if no > yes && no - yes >= theta {
                sn |= bit(x);
            }
        }
        (sp, sn)
    }

    /// The next object to ask: top up weak votes first, then ask a representative of an uncovered class.
    pub fn next_question(&self) -> Option<usize> {
        self.next_question_with(1)
    }

    /// Next question with a threshold: top up weak votes first (rule 2), then ask an uncovered class.
    pub fn next_question_with(&self, theta: u8) -> Option<usize> {
        if let Some(x) = self.pending(theta).into_iter().next() {
            return Some(x);
        }
        let (sp, sn) = self.strong(theta);
        let known = sp | sn;
        self.classes()
            .into_iter()
            .find(|c| c & known == 0)
            .map(|c| c.trailing_zeros() as usize)
    }

    /// Record the answer to one membership query (a single vote).
    pub fn answer(&mut self, x: usize, is_member: bool) {
        if is_member {
            self.answer_votes(x, 1, 0);
        } else {
            self.answer_votes(x, 0, 1);
        }
    }

    /// Record an answer with vote counts. The same object may vote several times.
    pub fn answer_votes(&mut self, x: usize, yes: u8, no: u8) {
        assert!(x < self.n);
        self.votes[x].0 = self.votes[x].0.saturating_add(yes);
        self.votes[x].1 = self.votes[x].1.saturating_add(no);
        self.asked |= bit(x);
        self.sync_plain_labels();
    }

    /// Members never asked directly: objects of a multi-member equivalence class that are not in `asked`.
    /// Their labels are 'inherited' from class members and may be wrong — probe one of each before stopping.
    pub fn unprobed(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for c in self.classes() {
            if c.count_ones() < 2 {
                continue;
            }
            for x in 0..self.n {
                if c & bit(x) != 0 && self.asked & bit(x) == 0 {
                    out.push(x);
                }
            }
        }
        out.sort_unstable(); // classes() goes through a HashMap, so the order is unstable; fix it here
        out
    }

    /// Maintain `pos` / `neg` from majority votes (used by label-reading paths such as `explain` and `cover`).
    fn sync_plain_labels(&mut self) {
        let (sp, sn) = self.strong(1);
        self.pos = sp;
        self.neg = sn;
    }

    /// Adopt a new predicate (vocabulary extension). The caller must ensure it is 'worth it' — this structure does no scoring.
    pub fn propose(&mut self, p: Predicate) {
        self.vocab.add(p);
    }

    /// T4 capacity: upper bound on the number of distinguishable definable sets on the sample = 2^(number of signature classes).
    pub fn capacity(&self) -> u64 {
        1u64.checked_shl(self.classes().len() as u32).unwrap_or(u64::MAX)
    }

    /// The settled answer set (for debugging).
    pub fn known(&self) -> Mask {
        self.pos | self.neg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 objects; predicate a separates {0,1} from {2,3}.
    fn ctx() -> Explorer {
        let a = Predicate::new("a", bit(0) | bit(1));
        Explorer::new(4, Vocabulary::with(vec![a]))
    }

    #[test]
    fn t2_insufficient_when_pos_and_neg_collide() {
        let mut e = ctx();
        // 0 and 1 are both true on a, so they share the same signature
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
    }

    #[test]
    fn proposing_a_separating_predicate_leaves_insufficiency() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);

        // b contains only object 0, separating 0 from 1
        e.propose(Predicate::new("b", bit(0)));
        assert_ne!(e.status(), Status::Insufficient);
    }

    /// Theorem 5 of the paper (T6 in the Lean file): insufficiency is never repaired by more examples.
    #[test]
    fn more_data_never_repairs_insufficiency() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
        e.answer(2, true);
        assert_eq!(e.status(), Status::Insufficient);
        e.answer(3, false);
        assert_eq!(e.status(), Status::Insufficient);
    }

    #[test]
    fn identification_loop_terminates() {
        let mut e = ctx();
        e.answer(0, true);
        e.answer(3, false);
        let mut guard = 0;
        while e.status() == Status::Undetermined {
            let x = e.next_question().expect("undetermined implies a free class");
            // target S = {0, 1}
            let member = x == 0 || x == 1;
            e.answer(x, member);
            guard += 1;
            assert!(guard <= 4, "loop must terminate within |X| steps");
        }
        assert_eq!(e.status(), Status::Identified);
    }

    #[test]
    fn t4_capacity_grows_with_splitting_predicate() {
        let mut e = ctx();
        let before = e.capacity();
        e.propose(Predicate::new("b", bit(0) | bit(2)));
        assert!(e.capacity() > before, "a splitting predicate must raise capacity");
    }

    #[test]
    fn votes_need_a_margin() {
        // two singleton classes: p contains only object 0
        let mut e = Explorer::new(2, Vocabulary::with(vec![Predicate::new("p", bit(0))]));
        e.answer_votes(0, 1, 1); // tied, not a confirmation
        e.answer_votes(1, 0, 3); // negative confirmed
        assert_eq!(e.status_with(1), Status::Undetermined); // 0 has no confirmed label yet
        assert_eq!(e.status_with(2), Status::Undetermined);

        e.answer_votes(0, 2, 1); // margin 1, still not confirmed at threshold 2
        assert_eq!(e.status_with(2), Status::Undetermined);

        e.answer_votes(0, 2, 0); // cumulative 3:1, margin 2
        assert_eq!(e.status_with(2), Status::Identified);
        assert_eq!(e.status_with(1), Status::Identified);
    }

    #[test]
    fn noise_can_be_outvoted() {
        let mut e = ctx(); // {0,1} are in the same class
        e.answer_votes(0, 5, 1); // strong positive
        e.answer_votes(1, 1, 5); // strong negative
        assert_eq!(e.status(), Status::Insufficient);
        // raise the threshold to 6: neither side is confirmed, so it drops back to undetermined instead of falsely reporting a collision
        assert_eq!(e.status_with(6), Status::Undetermined);
    }

    #[test]
    fn weak_vote_blocks_identification() {
        let mut e = Explorer::new(2, Vocabulary::with(vec![Predicate::new("p", bit(0))]));
        e.answer_votes(0, 3, 0); // strong positive
        e.answer_votes(1, 1, 1); // tied → weak vote, blocks 'identified'
        assert_eq!(e.status_with(2), Status::Undetermined);
        assert_eq!(e.pending(2), vec![1]);
        assert_eq!(e.next_question_with(2), Some(1)); // top up the weak vote first

        e.answer_votes(1, 0, 2); // cumulative (1,3), margin 2 → strong negative
        assert_eq!(e.status_with(2), Status::Identified);
        assert!(e.pending(2).is_empty());
    }

    #[test]
    fn epsilon_hat_and_theta() {
        let mut e = Explorer::new(4, Vocabulary::default());
        e.answer_votes(0, 3, 0);
        e.answer_votes(1, 0, 3);
        assert!(e.implied_epsilon() < 0.01);
        assert_eq!(e.recommended_theta(), 3);

        e.answer_votes(2, 2, 1); // minority vote 1/3
        e.answer_votes(3, 1, 2);
        assert!(e.implied_epsilon() > 0.15); // ≈ 0.167
        assert_eq!(e.recommended_theta(), 4); // falls between 0.1 and 0.2
    }

    #[test]
    fn asked_and_unprobed_track_direct_questions() {
        let mut e = Explorer::new(3, Vocabulary::default()); // a single class
        e.answer_votes(0, 3, 0);
        assert_eq!(e.asked, bit(0));
        assert_eq!(e.unprobed(), vec![1, 2]);
        e.answer_votes(1, 0, 3);
        assert_eq!(e.unprobed(), vec![2]);
    }

    /// Operand order is not part of a definition's identity: construction canonicalises and
    /// equality ignores order even for a literal that bypassed the constructor.
    #[test]
    fn definitions_compare_ignoring_operand_order() {
        let built = Definition::new(DefOp::AllOf, vec!["b".into(), "a".into()]);
        assert_eq!(built.operands, vec!["a", "b"], "new() stores canonical order");

        let literal = Definition {
            op: DefOp::AllOf,
            operands: vec!["b".into(), "a".into()],
        };
        assert_eq!(built, literal, "all_of:[b,a] and all_of:[a,b] are one definition");
        assert_ne!(
            built,
            Definition {
                op: DefOp::AnyOf,
                operands: vec!["a".into(), "b".into()],
            },
            "the operator is still part of the identity"
        );
    }

    #[test]
    fn empty_vocabulary_makes_every_pair_insufficient() {
        let mut e = Explorer::new(3, Vocabulary::default());
        // empty vocabulary: three objects share one signature, so there is one class and only two definable sets, {} and X
        assert_eq!(e.capacity(), 2);
        e.answer(0, true);
        e.answer(1, false);
        assert_eq!(e.status(), Status::Insufficient);
        e.propose(Predicate::new("p", bit(0)));
        // after adding p the classes become {0} and {1,2}: the former holds a positive, the latter a negative, so all are covered
        assert_eq!(e.status(), Status::Identified);
    }
}
