//! One round, every operator, one pool.
//!
//! The controller in `run` adopts at most one offer per step, which is right for making progress and
//! wrong for comparing components: the offers that lost are discarded with the step. This module
//! asks every operator once, keeps everything they return, and folds the candidates into one pool
//! keyed by extension, so a component is judged on all of its proposals rather than on the one that
//! happened to be adopted.
//!
//! Three rules, all of them checkable:
//!
//!   1. validation is the kernel's, not this module's: a candidate that the validator rejects is
//!      reported with its reason and never pooled as if it were usable;
//!   2. one extension is one row: identical extensions from different components merge, and every
//!      source is kept, because "two components found the same set" is a fact about the components;
//!   3. order is deterministic: more splits first, then the extension as a number, then the source
//!      name, so two runs over the same input produce identical bytes.

use metaform::Mask;
use crate::{splits, validate, Context, OpStatus, Operator};

/// One operator's run, whether or not it produced anything.
#[derive(Clone, Debug)]
pub struct OpReport {
    pub operator: String,
    pub status: OpStatus,
    pub cost: f32,
    pub proposed: usize,
    /// Why the operator declined or failed, when it did.
    pub note: String,
}

/// One distinct extension in the pool, with every component that proposed it.
#[derive(Clone, Debug)]
pub struct Pooled {
    pub extension: Mask,
    /// How many colliding pairs this extension separates.
    pub splits: usize,
    /// Component names, in the order they were asked, deduplicated.
    pub sources: Vec<String>,
    /// One human-readable line per source, aligned with `sources`.
    pub details: Vec<String>,
    /// False when the validator refused every candidate with this extension.
    pub usable: bool,
    /// The validator's reason, when the extension is not usable.
    pub reason: String,
}

/// The whole round: what each operator did, and the pool.
#[derive(Clone, Debug)]
pub struct Round {
    pub operators: Vec<OpReport>,
    pub pool: Vec<Pooled>,
}

/// Ask every operator once and fold the candidates into one pool.
pub fn collect(ctx: &Context, operators: &[Box<dyn Operator>], budget: usize) -> Round {
    let mut reports = Vec::with_capacity(operators.len());
    let mut pooled: Vec<Pooled> = Vec::new();

    for operator in operators {
        let name = operator.name().to_string();
        if !operator.applicable(ctx) {
            reports.push(OpReport {
                operator: name,
                status: OpStatus::NotApplicable,
                cost: operator.estimate_cost(ctx),
                proposed: 0,
                note: "declined: not applicable to this state".to_string(),
            });
            continue;
        }
        let result = operator.execute(ctx, budget);
        let verdict = validate(ctx, &result.candidates);
        let rejected: Vec<(Mask, String)> = verdict
            .rejected
            .iter()
            .map(|(candidate, reason)| (candidate.extension, reason.clone()))
            .collect();

        for candidate in &result.candidates {
            let reason = rejected
                .iter()
                .find(|(ext, _)| *ext == candidate.extension)
                .map(|(_, why)| why.clone());
            let usable = reason.is_none();
            let splits = splits(ctx, candidate.extension);
            match pooled.iter_mut().find(|p| p.extension == candidate.extension) {
                Some(existing) => {
                    if !existing.sources.iter().any(|s| *s == name) {
                        existing.sources.push(name.clone());
                        existing.details.push(candidate.detail.clone());
                    }
                    // A row is usable if any component's candidate for that extension passed.
                    if usable {
                        existing.usable = true;
                        existing.reason.clear();
                    }
                }
                None => pooled.push(Pooled {
                    extension: candidate.extension,
                    splits,
                    sources: vec![name.clone()],
                    details: vec![candidate.detail.clone()],
                    usable,
                    reason: reason.unwrap_or_default(),
                }),
            }
        }

        reports.push(OpReport {
            operator: name,
            status: result.status,
            cost: result.cost,
            proposed: result.candidates.len(),
            note: result.unresolved.first().cloned().unwrap_or_default(),
        });
    }

    pooled.sort_by(|a, b| {
        b.splits
            .cmp(&a.splits)
            .then(a.extension.cmp(&b.extension))
            .then_with(|| a.sources.first().cmp(&b.sources.first()))
    });
    Round {
        operators: reports,
        pool: pooled,
    }
}

impl Round {
    /// The pool as one JSON value, with a fixed key order.
    pub fn to_json(&self) -> serde_json::Value {
        let operators: Vec<serde_json::Value> = self
            .operators
            .iter()
            .map(|r| {
                serde_json::json!({
                    "operator": r.operator,
                    "status": match r.status {
                        OpStatus::Applied => "applied",
                        OpStatus::NotApplicable => "not_applicable",
                        OpStatus::Failed => "failed",
                    },
                    "cost": (r.cost * 1000.0).round() / 1000.0,
                    "proposed": r.proposed,
                    "note": r.note,
                })
            })
            .collect();
        let pool: Vec<serde_json::Value> = self
            .pool
            .iter()
            .map(|p| {
                serde_json::json!({
                    "extension": p.extension,
                    "ids": ids(p.extension),
                    "splits": p.splits,
                    "sources": p.sources,
                    "details": p.details,
                    "usable": p.usable,
                    "reason": p.reason,
                })
            })
            .collect();
        serde_json::json!({
            "operators": operators,
            "pool": pool,
            "distinct": self.pool.len(),
            "usable": self.pool.iter().filter(|p| p.usable).count(),
        })
    }
}

fn ids(mask: Mask) -> Vec<usize> {
    (0..64).filter(|i| mask & (1u64 << i) != 0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Candidate;
    use metaform::{bit, Vocabulary};

    struct Stub {
        name: &'static str,
        ext: Mask,
        status: OpStatus,
    }

    impl Operator for Stub {
        fn name(&self) -> &'static str {
            self.name
        }
        fn applicable(&self, _ctx: &Context) -> bool {
            self.status != OpStatus::NotApplicable
        }
        fn estimate_cost(&self, _ctx: &Context) -> f32 {
            1.0
        }
        fn execute(&self, _ctx: &Context, _budget: usize) -> crate::OperatorResult {
            if self.status == OpStatus::Failed {
                return crate::OperatorResult {
                    candidates: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    unresolved: vec!["no clause within the limit".to_string()],
                    cost: 1.0,
                    status: OpStatus::Failed,
                };
            }
            crate::OperatorResult {
                candidates: vec![Candidate {
                    name: self.name.to_string(),
                    extension: self.ext,
                    source: self.name.to_string(),
                    detail: format!("{} proposes {}", self.name, self.ext),
                    warrant: vec!["stub".to_string()],
                    falsifier: vec!["a contradicting label".to_string()],
                    adoptable: true,
                }],
                evidence: Vec::new(),
                assumptions: Vec::new(),
                unresolved: Vec::new(),
                cost: 1.0,
                status: OpStatus::Applied,
            }
        }
    }

    fn ctx() -> Context {
        // Four objects: 0,1 positive and 2,3 negative, with no predicates at all, so the two classes
        // collide and every non-constant extension is worth something.
        Context {
            objects: 4,
            vocabulary: Vocabulary::default(),
            attributes: vec![("a".to_string(), bit(0) | bit(1))],
            pos: bit(0) | bit(1),
            neg: bit(2) | bit(3),
            label: "pool-test".to_string(),
        }
    }

    #[test]
    fn four_operators_go_through_one_path_and_identical_extensions_merge() {
        let operators: Vec<Box<dyn Operator>> = vec![
            Box::new(Stub { name: "alpha", ext: bit(0) | bit(1), status: OpStatus::Applied }),
            Box::new(Stub { name: "beta", ext: bit(0) | bit(1), status: OpStatus::Applied }),
            Box::new(Stub { name: "gamma", ext: bit(0), status: OpStatus::Applied }),
            Box::new(Stub { name: "delta", ext: 0, status: OpStatus::Failed }),
        ];
        let round = collect(&ctx(), &operators, 4);
        assert_eq!(round.pool.len(), 2, "{:?}", round.pool);
        let merged = round
            .pool
            .iter()
            .find(|p| p.extension == bit(0) | bit(1))
            .expect("merged row");
        assert_eq!(merged.sources, vec!["alpha", "beta"]);
        assert!(merged.usable);
        assert_eq!(round.operators.len(), 4);
        assert_eq!(round.operators[3].status, OpStatus::Failed);
    }

    #[test]
    fn a_declined_operator_is_reported_and_never_pooled() {
        let operators: Vec<Box<dyn Operator>> =
            vec![Box::new(Stub { name: "alpha", ext: 1, status: OpStatus::NotApplicable })];
        let round = collect(&ctx(), &operators, 4);
        assert!(round.pool.is_empty());
        assert_eq!(round.operators[0].status, OpStatus::NotApplicable);
        assert!(round.operators[0].note.starts_with("declined"));
    }

    #[test]
    fn the_pool_is_ordered_by_splits_then_extension_and_is_deterministic() {
        let operators: Vec<Box<dyn Operator>> = vec![
            Box::new(Stub { name: "wide", ext: bit(0), status: OpStatus::Applied }),
            Box::new(Stub { name: "perfect", ext: bit(0) | bit(1), status: OpStatus::Applied }),
        ];
        let first = collect(&ctx(), &operators, 4).to_json().to_string();
        let second = collect(&ctx(), &operators, 4).to_json().to_string();
        assert_eq!(first, second, "the same input must produce the same bytes");
        let round = collect(&ctx(), &operators, 4);
        assert_eq!(round.pool[0].splits, round.pool[0].splits.max(round.pool[1].splits));
        assert_eq!(round.pool[0].extension, bit(0) | bit(1));
    }
}
