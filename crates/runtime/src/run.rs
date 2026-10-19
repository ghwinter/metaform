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
    adopt, colliding_pairs, status_with, validate, Candidate, Context, Operator,
};
use metaform::Status;

/// One operator's offer during a step.
#[derive(Clone, Debug)]
pub struct Offer {
    pub operator: String,
    pub name: String,
    pub splits: usize,
    pub cost: f32,
    pub adopted: bool,
    /// The operator's own words about this offer, carried through for the report.
    pub note: String,
}

/// What one step did: every offer it saw, and which one was adopted.
#[derive(Clone, Debug)]
pub struct Step {
    pub index: usize,
    pub status_before: Status,
    pub status_after: Status,
    pub colliding_before: usize,
    pub vocabulary_before: usize,
    pub offers: Vec<Offer>,
    /// A label the probe had to disbelieve, when the adopted proposal could not separate everything.
    pub conflict: Option<String>,
}

impl Step {
    pub fn adopted(&self) -> Option<&Offer> {
        self.offers.iter().find(|o| o.adopted)
    }
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
        let vocabulary_before = ctx.vocabulary.len();

        // Ask every applicable operator, validate everything it offered, and keep every offer that
        // separates at least one pair -- including the ones that will not be adopted, because the
        // record is only useful if it can see what lost.
        let mut offers: Vec<Offer> = Vec::new();
        let mut seen: Vec<(Offer, Candidate)> = Vec::new();
        for op in operators {
            if !op.applicable(ctx) {
                continue;
            }
            let result = op.execute(ctx, 4);
            let verdict = validate(ctx, &result.candidates);
            for candidate in verdict.accepted {
                let splits = crate::splits(ctx, candidate.extension);
                if splits > 0 {
                    let offer = Offer {
                        operator: op.name().to_string(),
                        name: candidate.name.clone(),
                        splits,
                        cost: result.cost,
                        adopted: false,
                        note: candidate.warrant.first().cloned().unwrap_or_default(),
                    };
                    offers.push(offer.clone());
                    seen.push((offer, candidate));
                }
            }
        }

        // Deterministic choice: most pairs separated, then cheapest, then name.
        offers.sort_by(|a, b| {
            b.splits
                .cmp(&a.splits)
                .then(a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.name.cmp(&b.name))
        });

        if let Some(best) = offers.first().cloned() {
            if let Some((_, candidate)) =
                seen.iter().find(|(o, _)| o.name == best.name && o.operator == best.operator)
            {
                adopt(ctx, candidate);
            }
            let status_after = status_with(ctx, &[]);
            let mut recorded = offers.clone();
            for offer in recorded.iter_mut() {
                offer.adopted = offer.name == best.name && offer.operator == best.operator;
            }
            // If the best offer still leaves pairs unexplained, anything the probe said about a
            // doubtful label belongs in the report: the state is not merely undecided, it is
            // contradictory under the labels as given.
            let conflict = if best.splits < pairs.len() {
                let notes: Vec<String> = recorded
                    .iter()
                    .filter(|o| o.operator == "probe-labels" && !o.note.is_empty())
                    .map(|o| format!("{} {}", o.name, o.note))
                    .collect();
                if notes.is_empty() {
                    None
                } else {
                    Some(notes.join("; "))
                }
            } else {
                None
            };
            steps.push(Step {
                index,
                status_before,
                status_after,
                colliding_before: pairs.len(),
                vocabulary_before,
                offers: recorded,
                conflict,
            });
        } else {
            // Nothing on the table separates a pair: record what was seen and stop rather than loop.
            let conflict = offers
                .iter()
                .any(|o| o.operator == "probe-labels")
                .then(|| {
                    offers
                        .iter()
                        .filter(|o| o.operator == "probe-labels" && !o.note.is_empty())
                        .map(|o| format!("{} {}", o.name, o.note))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .filter(|s| !s.is_empty());
            steps.push(Step {
                index,
                status_before,
                status_after: status_before,
                colliding_before: pairs.len(),
                vocabulary_before,
                offers,
                conflict,
            });
            break;
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
            label: String::new(),
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
        assert!(steps[0].adopted().is_some(), "the first step should adopt something");
        assert!(steps[0].adopted().unwrap().splits > 0);
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
            fn execute(&self, _: &Context, _: usize) -> crate::OperatorResult {
                crate::OperatorResult {
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
        assert!(steps[0].adopted().is_none());
        assert!(steps[0].offers.is_empty(), "nothing separated, so nothing was offered");
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
