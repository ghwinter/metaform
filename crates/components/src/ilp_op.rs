//! Clause induction: learn one clause that covers the labelled positives and no negative.
//!
//! With the context this operator is given today, every attribute is unary, so a learned clause is a
//! conjunction of attributes and its extension equals the `all_of` candidates `suggest` enumerates.
//! That is stated here rather than hidden: the operator's value right now is that it goes through the
//! same contract as the other components, and that it is the placement for relational learning the
//! day the context carries relations. What it adds even today is a *minimal* body (shortest first)
//! and a countable justification, not a different hypothesis space.

use metaform::{bit, Mask};
use metaform_ilp::{extension, learn, Atom, Background, Clause, Signature, Term};
use metaform_runtime::{colliding_pairs, Candidate, Context, OpStatus, Operator, OperatorResult};

/// Learns the first clause, shortest body first, that covers every labelled positive and no labelled
/// negative.
pub struct IlpInducer {
    /// Longest body the learner may try.
    pub max_body: usize,
    /// Variables the body may use; one is enough for a unary target.
    pub max_vars: u32,
    /// Name of the target predicate in the learned clause.
    pub target: String,
}

impl Default for IlpInducer {
    fn default() -> Self {
        IlpInducer {
            max_body: 3,
            max_vars: 1,
            target: "concept".to_string(),
        }
    }
}

fn atoms_of(mask: Mask, objects: usize, pred: &str) -> Vec<Atom> {
    (0..objects)
        .filter(|&o| mask & bit(o) != 0)
        .map(|o| Atom {
            pred: pred.to_string(),
            args: vec![Term::Const(o as u32)],
        })
        .collect()
}

fn background(ctx: &Context) -> Background {
    let mut bg = Background::new(
        ctx.attributes
            .iter()
            .map(|(name, _)| Signature {
                name: name.clone(),
                arity: 1,
            })
            .collect(),
    );
    for (name, mask) in &ctx.attributes {
        for o in 0..ctx.objects {
            if mask & bit(o) != 0 {
                bg.fact(name, &[o as u32]);
            }
        }
    }
    bg
}

fn show(clause: &Clause) -> String {
    let body: Vec<String> = clause
        .body
        .iter()
        .map(|atom| {
            let args: Vec<String> = atom
                .args
                .iter()
                .map(|t| match t {
                    Term::Var(v) => format!("V{v}"),
                    Term::Const(c) => c.to_string(),
                })
                .collect();
            format!("{}({})", atom.pred, args.join(","))
        })
        .collect();
    format!("{} :- {}", clause.head.pred, body.join(", "))
}

impl Operator for IlpInducer {
    fn name(&self) -> &'static str {
        "ilp-inducer"
    }

    fn applicable(&self, ctx: &Context) -> bool {
        !colliding_pairs(ctx).is_empty()
            && !ctx.attributes.is_empty()
            && ctx.pos & ctx.neg == 0
            && ctx.pos != 0
            && ctx.neg != 0
    }

    fn estimate_cost(&self, ctx: &Context) -> f32 {
        // Order of magnitude: the number of literals to the body length, over the fact base.
        1.0 + (ctx.attributes.len() as f32).powi(self.max_body as i32) / 1e3
    }

    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult {
        if !self.applicable(ctx) {
            return OperatorResult::not_applicable(
                "needs a collision, at least one attribute, and both a positive and a negative example",
            );
        }
        if budget == 0 {
            return OperatorResult::not_applicable("budget is zero");
        }
        let bg = background(ctx);
        let positives = atoms_of(ctx.pos, ctx.objects, &self.target);
        let negatives = atoms_of(ctx.neg, ctx.objects, &self.target);
        let domain: Vec<u32> = (0..ctx.objects as u32).collect();

        match learn(
            &self.target,
            1,
            &positives,
            &negatives,
            &bg,
            self.max_body,
            self.max_vars,
        ) {
            Some(clause) => {
                let tuples = extension(&clause, &bg, &domain);
                let mut ext: Mask = 0;
                for tuple in tuples {
                    if let Some(&o) = tuple.first() {
                        ext |= bit(o as usize);
                    }
                }
                let candidate = Candidate {
                    name: format!("ilp_rule_{}", self.max_body),
                    extension: ext,
                    source: self.name().to_string(),
                    detail: show(&clause),
                    warrant: vec![
                        format!("covers all {} labelled positive(s)", positives.len()),
                        format!("covers none of the {} labelled negative(s)", negatives.len()),
                        "body is the shortest that does so".to_string(),
                    ],
                    falsifier: vec![
                        "a labelled object whose class contradicts the clause extension".to_string(),
                    ],
                    adoptable: true,
                };
                OperatorResult {
                    candidates: vec![candidate],
                    evidence: vec![format!(
                        "{} positive and {} negative example(s), {} attribute(s)",
                        positives.len(),
                        negatives.len(),
                        ctx.attributes.len()
                    )],
                    assumptions: vec![
                        "attributes are unary, so the learned clause is a conjunction".to_string(),
                        "the examples are noise-free".to_string(),
                    ],
                    unresolved: Vec::new(),
                    cost: self.estimate_cost(ctx),
                    status: OpStatus::Applied,
                }
            }
            None => OperatorResult {
                candidates: Vec::new(),
                evidence: vec![format!(
                    "{} attribute(s) tried up to body length {}",
                    ctx.attributes.len(),
                    self.max_body
                )],
                assumptions: Vec::new(),
                unresolved: vec![
                    "no clause within the body limit covers every positive without a negative"
                        .to_string(),
                ],
                cost: self.estimate_cost(ctx),
                status: OpStatus::Failed,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use metaform::Vocabulary;

    fn ctx() -> Context {
        // Four objects; "red" holds of 0 and 1, "big" of 1 and 3. The positives are {0,1} and the
        // negatives {2,3}, so no single attribute separates them but "red" alone does not either:
        // the learned clause must be `concept :- red` covering 0,1 and excluding 2,3.
        let attributes = vec![
            ("red".to_string(), bit(0) | bit(1)),
            ("big".to_string(), bit(1) | bit(3)),
        ];
        Context {
            objects: 4,
            vocabulary: Vocabulary::default(),
            attributes,
            pos: bit(0) | bit(1),
            neg: bit(2) | bit(3),
            label: "ilp-test".to_string(),
        }
    }

    #[test]
    fn learns_a_clause_that_covers_positives_and_excludes_negatives() {
        let op = IlpInducer::default();
        let result = op.execute(&ctx(), 4);
        assert_eq!(result.status, OpStatus::Applied, "{:?}", result.unresolved);
        assert_eq!(result.candidates.len(), 1);
        let candidate = &result.candidates[0];
        assert_eq!(candidate.extension, bit(0) | bit(1));
        assert!(candidate.detail.starts_with("concept :- "), "{}", candidate.detail);
        assert!(candidate.adoptable);
    }

    #[test]
    fn refuses_when_there_is_nothing_to_learn_from() {
        let mut c = ctx();
        // No attributes: the body signature is empty, so the operator must decline rather than
        // return an empty clause that covers everything.
        c.attributes.clear();
        let result = IlpInducer::default().execute(&c, 4);
        assert_eq!(result.status, OpStatus::NotApplicable);
    }

    #[test]
    fn refuses_a_zero_budget() {
        let result = IlpInducer::default().execute(&ctx(), 0);
        assert_eq!(result.status, OpStatus::NotApplicable);
    }
}
