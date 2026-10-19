//! The controller: run the operators against a state until it is decided or nothing helps.
//!
//! This is the part of the runtime that the paper's alternation corollary prescribes: while the
//! vocabulary is insufficient, ask the operators for a predicate; adopt the proposal that separates
//! the most colliding pairs; stop when nothing separates anything or the state is no longer
//! insufficient. Every step is recorded, so a sequence of runs can be inspected afterwards.
//!
//! The controller never invents a proposal and never adopts one the validator rejected. It only
//! chooses among what the components offered.

use crate::{
    adopt, colliding_pairs, status_with, validate, Candidate, Context, Operator, OperatorResult,
};
use metaform::Status;

/// What one step did.
#[derive(Clone, Debug)]
pub struct Step {
    pub index: usize,
    pub status_before: Status,
    pub status_after: Status,
    /// Operator that produced the adopted proposal, or the one that tried and failed.
    pub operator: String,
    pub proposal: String,
    pub splits: usize,
    pub colliding_before: usize,
    pub vocabulary_before: usize,
    pub adopted: bool,
    pub cost: f32,
}

/// A proposal on the table.
struct Offered {
    candidate: Candidate,
    operator: String,
    splits: usize,
    cost: f32,
}

/// Runs the loop. Returns one record per step, including the final step that stopped it.
pub fn run(ctx: &mut Context, operators: &[Box<dyn Operator>], max_steps: usize) -> Vec<Step> {
    let mut steps = Vec::new();

    for index in 0..max_steps {
        let status_before = status_with(ctx, &[]);
        let pairs = colliding_pairs(ctx);
        if status_before != Status::Insufficient || pairs.is_empty() {
            break;
        }

        // Ask every applicable operator, validate everything it offered, keep what separates.
        let mut offers: Vec<Offered> = Vec::new();
        let mut tried: Vec<(String, OperatorResult)> = Vec::new();
        for op in operators {
            if !op.applicable(ctx) {
                continue;
            }
            let result = op.execute(ctx, 4);
            let verdict = validate(ctx, &result.candidates);
            let cost = result.cost;
            tried.push((op.name().to_string(), result));
            for candidate in verdict.accepted {
                let splits = crate::splits(ctx, candidate.extension);
                if splits > 0 {
                    offers.push(Offered {
                        candidate,
                        operator: op.name().to_string(),
                        splits,
                        cost,
                    });
                }
            }
        }

        // Deterministic choice: most pairs separated, then cheapest, then name.
        offers.sort_by(|a, b| {
            b.splits
                .cmp(&a.splits)
                .then(a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.candidate.name.cmp(&b.candidate.name))
        });

        match offers.first() {
            Some(best) => {
                let candidate = best.candidate.clone();
                let vocabulary_before = ctx.vocabulary.len();
                adopt(ctx, &candidate);
                let status_after = status_with(ctx, &[]);
                steps.push(Step {
                    index,
                    status_before,
                    status_after,
                    operator: best.operator.clone(),
                    proposal: candidate.name.clone(),
                    splits: best.splits,
                    colliding_before: pairs.len(),
                    vocabulary_before,
                    adopted: true,
                    cost: best.cost,
                });
            }
            None => {
                // Nothing on the table separates a pair: record it and stop rather than loop.
                let (operator, cost, name) = tried
                    .iter()
                    .find_map(|(name, result)| {
                        result.candidates.first().map(|c| (name.clone(), result.cost, c.name.clone()))
                    })
                    .unwrap_or_else(|| ("none".to_string(), 0.0, "no proposal".to_string()));
                steps.push(Step {
                    index,
                    status_before,
                    status_after: status_before,
                    operator,
                    proposal: name,
                    splits: 0,
                    colliding_before: pairs.len(),
                    vocabulary_before: ctx.vocabulary.len(),
                    adopted: false,
                    cost,
                });
                break;
            }
        }
    }

    steps
}

/// A compact, deterministic fingerprint of the state, for grouping runs.
pub fn fingerprint(ctx: &Context) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |byte: u8| {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    eat(ctx.objects as u8);
    eat(ctx.pos as u8);
    eat(ctx.neg as u8);
    for p in &ctx.vocabulary.preds {
        for b in p.name.bytes() {
            eat(b);
        }
        for k in 0..8 {
            eat((p.ext >> (k * 8)) as u8);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{label, SmeAnalogizer, VsaRetriever};
    use metaform::{bit, Predicate, Vocabulary};

    fn collision() -> Context {
        // Four objects; the vocabulary holds a, so 0/1 (positive) and 2 (negative) collide.
        let a = bit(0) | bit(1) | bit(2);
        let b = bit(0) | bit(1);
        let c = bit(3);
        let mut ctx = Context {
            objects: 4,
            vocabulary: Vocabulary::with(vec![Predicate::new("a", a)]),
            attributes: vec![
                ("a".to_string(), a),
                ("b".to_string(), b),
                ("c".to_string(), c),
            ],
            pos: 0,
            neg: 0,
        };
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, true);
        label(&mut ctx, 2, false);
        label(&mut ctx, 3, false);
        ctx
    }

    #[test]
    fn the_controller_adopts_a_separating_proposal_and_stops() {
        let mut ctx = collision();
        let systems = crate::scenarios::water_and_heat();
        let operators: Vec<Box<dyn Operator>> = vec![
            Box::new(SmeAnalogizer {
                base: systems.0,
                target: systems.1,
                readings: vec![("liquid".to_string(), bit(0) | bit(1))],
            }),
            Box::new(VsaRetriever { dim: 2048, seed: 7, max_arity: 2 }),
        ];
        let steps = run(&mut ctx, &operators, 4);
        assert!(!steps.is_empty());
        assert!(steps[0].adopted, "the first step should adopt something");
        assert!(steps[0].splits > 0);
        assert!(colliding_pairs(&ctx).is_empty(), "the collision should be gone");
    }

    #[test]
    fn the_controller_stops_when_nothing_helps() {
        // One operator that always proposes a predicate separating nothing.
        struct Useless;
        impl Operator for Useless {
            fn name(&self) -> &'static str {
                "useless"
            }
            fn applicable(&self, _: &Context) -> bool {
                true
            }
            fn estimate_cost(&self, _: &Context) -> f32 {
                1.0
            }
            fn execute(&self, _: &Context, _: usize) -> OperatorResult {
                OperatorResult {
                    candidates: vec![Candidate {
                        name: "constant".to_string(),
                        extension: 0,
                        source: "useless".to_string(),
                        detail: String::new(),
                        warrant: Vec::new(),
                        falsifier: Vec::new(),
                    }],
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    unresolved: Vec::new(),
                    cost: 1.0,
                    status: crate::OpStatus::Applied,
                }
            }
        }
        let mut ctx = collision();
        let operators: Vec<Box<dyn Operator>> = vec![Box::new(Useless)];
        let steps = run(&mut ctx, &operators, 4);
        assert_eq!(steps.len(), 1);
        assert!(!steps[0].adopted);
        assert_eq!(steps[0].splits, 0);
    }

    #[test]
    fn the_fingerprint_changes_with_the_state() {
        let a = collision();
        let mut b = collision();
        let before = fingerprint(&a);
        label(&mut b, 3, true);
        assert_ne!(before, fingerprint(&b));
    }
}
