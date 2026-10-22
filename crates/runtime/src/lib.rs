//! The operator protocol: proposal components sit behind the exact kernel.
//!
//! Two rules from the runtime design are enforced here rather than documented:
//!
//!   1. A component returns proposals with their provenance; the kernel is what decides. Nothing a
//!      component returns changes the state until `validate` accepts it and the caller adopts it.
//!   2. Results keep their semantics. A structural mapping, a vector similarity and a coverage
//!      count are different things and are never added together into one score.
//!
//! The kernel itself (the `metaform` crate) does not depend on this crate or on any component.

use metaform::{bit, Explorer, Mask, Predicate, Status, Vocabulary};

/// What a component says about its own run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpStatus {
    /// The component produced proposals.
    Applied,
    /// The component had nothing to do on this state.
    NotApplicable,
    /// The component tried and failed; the reason is in `unresolved`.
    Failed,
}

/// A proposed predicate: an extension plus where it came from and what would refute it.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub extension: Mask,
    /// Which component proposed it, kept separate from any score.
    pub source: String,
    /// The component's own reading of the proposal (a mapping, a similarity, a clause), unmodified.
    pub detail: String,
    pub warrant: Vec<String>,
    pub falsifier: Vec<String>,
    /// False when the proposal is a report rather than something the state should adopt: a
    /// conflict against a doubted label, for instance, is information for the caller, not a
    /// predicate to add.
    pub adoptable: bool,
}

/// What an operator returns. The fields are deliberately not collapsed into a single number.
#[derive(Clone, Debug)]
pub struct OperatorResult {
    pub candidates: Vec<Candidate>,
    pub evidence: Vec<String>,
    pub assumptions: Vec<String>,
    pub unresolved: Vec<String>,
    pub cost: f32,
    pub status: OpStatus,
}

impl OperatorResult {
    pub fn not_applicable(why: &str) -> Self {
        OperatorResult {
            candidates: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            unresolved: vec![why.to_string()],
            cost: 0.0,
            status: OpStatus::NotApplicable,
        }
    }
}

/// The state an operator sees. It is a snapshot: an operator cannot change it.
#[derive(Clone, Debug)]
pub struct Context {
    pub objects: usize,
    /// The vocabulary currently in force.
    pub vocabulary: Vocabulary,
    /// Ground attributes the scenario provides, as extensions.
    pub attributes: Vec<(String, Mask)>,
    pub pos: Mask,
    pub neg: Mask,
    /// A caller-supplied name for this state, used by file-backed proposers and by traces.
    pub label: String,
}

impl Context {
    pub fn attribute(&self, name: &str) -> Option<Mask> {
        self.attributes.iter().find(|(n, _)| n == name).map(|(_, m)| *m)
    }
}

/// One proposal component.
pub trait Operator {
    fn name(&self) -> &'static str;

    /// Whether the component has anything to do on this state.
    fn applicable(&self, ctx: &Context) -> bool;

    /// The component's own cost estimate, in the caller's units.
    fn estimate_cost(&self, ctx: &Context) -> f32;

    /// Propose at most `budget` candidates. Must not change `ctx`.
    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult;
}

/// The pairs the current vocabulary cannot separate: one positive and one negative object with the
/// same signature. A nonempty list is the kernel's proof that the vocabulary is insufficient.
pub fn colliding_pairs(ctx: &Context) -> Vec<(usize, usize)> {
    let mut ex = Explorer::new(ctx.objects, ctx.vocabulary.clone());
    for x in 0..ctx.objects {
        let in_pos = (ctx.pos >> x) & 1 == 1;
        let in_neg = (ctx.neg >> x) & 1 == 1;
        if in_pos {
            ex.answer(x, true);
        } else if in_neg {
            ex.answer(x, false);
        }
    }
    let sigs: Vec<Vec<bool>> = (0..ctx.objects).map(|x| ex.signature(x)).collect();
    let mut pairs = Vec::new();
    for p in 0..ctx.objects {
        for n in 0..ctx.objects {
            if (ctx.pos >> p) & 1 == 1 && (ctx.neg >> n) & 1 == 1 && sigs[p] == sigs[n] {
                pairs.push((p, n));
            }
        }
    }
    pairs
}

/// The kernel's verdict on a batch of proposals.
#[derive(Clone, Debug, Default)]
pub struct Verdict {
    pub accepted: Vec<Candidate>,
    pub rejected: Vec<(Candidate, String)>,
}

impl Verdict {
    pub fn accepted_extensions(&self) -> Vec<Mask> {
        self.accepted.iter().map(|c| c.extension).collect()
    }
}

/// Checks proposals against the state, exactly as the kernel does for a model's proposals: an
/// extension must stay inside the object range, must not be empty or constant, and must not repeat
/// the vocabulary or an earlier proposal. The number of colliding pairs it separates is reported
/// separately — it is a scheduling score, not a correctness score.
pub fn validate(ctx: &Context, candidates: &[Candidate]) -> Verdict {
    let mut verdict = Verdict::default();
    let all = if ctx.objects >= 64 { u64::MAX } else { (1u64 << ctx.objects) - 1 };
    let existing: Vec<Mask> = ctx.vocabulary.preds.iter().map(|p| p.ext).collect();

    for c in candidates {
        let ext = c.extension;
        let reason = if ext == 0 {
            Some("empty extension")
        } else if ext == all {
            Some("constant extension")
        } else if ext & !all != 0 {
            Some("ids outside the object range")
        } else if existing.contains(&ext) {
            Some("duplicates a predicate already in the vocabulary")
        } else if verdict.accepted.iter().any(|a| a.extension == ext) {
            Some("duplicates an earlier accepted proposal")
        } else {
            None
        };
        match reason {
            Some(r) => verdict.rejected.push((c.clone(), r.to_string())),
            None => verdict.accepted.push(c.clone()),
        }
    }
    verdict
}

/// Adopts an accepted proposal into the state. This is the only place a proposal becomes state.
pub fn adopt(ctx: &mut Context, candidate: &Candidate) {
    ctx.vocabulary.add(Predicate::new(candidate.name.clone(), candidate.extension));
}

/// The status of the state after the accepted proposals are adopted, for reporting.
pub fn status_with(ctx: &Context, extra: &[Mask]) -> Status {
    let mut vocab = ctx.vocabulary.clone();
    for (i, ext) in extra.iter().enumerate() {
        vocab.add(Predicate::new(format!("extra{}", i), *ext));
    }
    let mut ex = Explorer::new(ctx.objects, vocab);
    for x in 0..ctx.objects {
        if (ctx.pos >> x) & 1 == 1 {
            ex.answer(x, true);
        } else if (ctx.neg >> x) & 1 == 1 {
            ex.answer(x, false);
        }
    }
    ex.status()
}

/// How many of the colliding pairs a proposed extension separates.
pub fn splits(ctx: &Context, ext: Mask) -> usize {
    colliding_pairs(ctx)
        .into_iter()
        .filter(|(p, n)| ((ext >> p) & 1) != ((ext >> n) & 1))
        .count()
}

/// Convenience for a small context.
pub fn context(objects: usize, vocabulary: Vocabulary, attributes: Vec<(String, Mask)>) -> Context {
    Context { objects, vocabulary, attributes, pos: 0, neg: 0, label: String::new() }
}

/// Marks an object as positive or negative in the context.
pub fn label(ctx: &mut Context, object: usize, positive: bool) {
    if positive {
        ctx.pos |= bit(object);
    } else {
        ctx.neg |= bit(object);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collision_context() -> Context {
        // Objects 0..6; a is in the vocabulary, b is not, so 0 and 1 collide.
        let a: Mask = bit(0) | bit(1) | bit(3);
        let ctx = Context {
            objects: 6,
            vocabulary: Vocabulary::with(vec![Predicate::new("a", a)]),
            attributes: vec![
                ("a".to_string(), a),
                ("b".to_string(), bit(1) | bit(2) | bit(5)),
                ("c".to_string(), bit(0) | bit(4)),
            ],
            pos: 0,
            neg: 0,
            label: String::new(),
        };
        ctx
    }

    #[test]
    fn the_collision_is_found() {
        let mut ctx = collision_context();
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, false);
        assert_eq!(colliding_pairs(&ctx), vec![(0, 1)]);
        assert_eq!(status_with(&ctx, &[]), Status::Insufficient);
    }

    #[test]
    fn validation_rejects_empty_constant_and_out_of_range() {
        let ctx = collision_context();
        let cand = |name: &str, ext: Mask| Candidate {
            name: name.to_string(),
            extension: ext,
            source: "test".to_string(),
            detail: String::new(),
            warrant: Vec::new(),
            falsifier: Vec::new(),
            adoptable: true,
        };
        let verdict = validate(
            &ctx,
            &[
                cand("ok", bit(2)),
                cand("empty", 0),
                cand("constant", (1 << 6) - 1),
                cand("outside", bit(9)),
                cand("repeats-a", bit(0) | bit(1) | bit(3)),
            ],
        );
        assert_eq!(verdict.accepted.len(), 1);
        assert_eq!(verdict.rejected.len(), 4);
    }

    #[test]
    fn a_predicate_that_separates_the_pair_is_counted() {
        let mut ctx = collision_context();
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, false);
        assert_eq!(splits(&ctx, bit(0)), 1); // b holds for 1 but not 0
        assert_eq!(splits(&ctx, bit(4)), 0); // c holds for neither
    }

    #[test]
    fn adopting_the_proposal_makes_the_state_identified() {
        let mut ctx = collision_context();
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, false);
        let proposal = Candidate {
            name: "b".to_string(),
            extension: ctx.attribute("b").unwrap(),
            source: "test".to_string(),
            detail: String::new(),
            warrant: Vec::new(),
            falsifier: Vec::new(),
            adoptable: true,
        };
        let verdict = validate(&ctx, std::slice::from_ref(&proposal));
        assert_eq!(verdict.accepted.len(), 1);
        adopt(&mut ctx, &verdict.accepted[0]);
        assert_ne!(status_with(&ctx, &[]), Status::Insufficient);
    }

}

/// Small relational systems used by the demo and the tests.
pub mod experience;
pub mod file_proposer;
pub mod record;
pub mod pool;
pub mod run;
pub mod state;
