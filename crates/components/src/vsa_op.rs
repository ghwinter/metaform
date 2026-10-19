//! Vector retrieval: rank candidate predicates by similarity to a labelled contrast.

use metaform_runtime::{colliding_pairs, Candidate, Context, OpStatus, Operator, OperatorResult};

/// A retrieval component: ranks candidate predicates by vector similarity to the contrast between
/// a positive and a negative object. It proposes; it never decides.
pub struct VsaRetriever {
    pub dim: usize,
    pub seed: u64,
    /// Largest conjunction the retriever may propose.
    pub max_arity: usize,
}

impl Operator for VsaRetriever {
    fn name(&self) -> &'static str {
        "vsa-retriever"
    }

    fn applicable(&self, ctx: &Context) -> bool {
        !colliding_pairs(ctx).is_empty() && !ctx.attributes.is_empty()
    }

    fn estimate_cost(&self, ctx: &Context) -> f32 {
        // One vector per candidate, ranking is linear in the pool size.
        let pool = ctx.attributes.len() + ctx.attributes.len() * ctx.attributes.len().saturating_sub(1) / 2;
        pool as f32 * self.dim as f32 / 1e6
    }

    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult {
        if !self.applicable(ctx) {
            return OperatorResult::not_applicable("no collision to break, or no attributes to rank");
        }
        let (pos, neg) = first_colliding_pair(ctx).expect("applicable checked");
        let ranked = metaform_vsa::retrieval::rank_for_pair(
            ctx.objects,
            &ctx.attributes,
            pos,
            neg,
            self.dim,
            self.seed,
            self.max_arity,
        );
        let candidates: Vec<Candidate> = ranked
            .into_iter()
            .take(budget)
            .map(|r| Candidate {
                name: r.name.clone(),
                extension: r.extension,
                source: self.name().to_string(),
                detail: format!("cosine {:.3} to the contrast of {} and {}", r.similarity, pos, neg),
                warrant: vec![format!("declared similar to the contrast between {} and {}", pos, neg)],
                falsifier: vec!["a labelled object that contradicts the extension".to_string()],
                adoptable: true,
            })
            .collect();
        OperatorResult {
            candidates,
            evidence: vec![format!("colliding pair ({}, {})", pos, neg)],
            assumptions: vec!["the vector encoding reflects attribute co-occurrence, not truth".to_string()],
            unresolved: Vec::new(),
            cost: self.estimate_cost(ctx),
            status: OpStatus::Applied,
        }
    }
}

fn first_colliding_pair(ctx: &Context) -> Option<(usize, usize)> {
    colliding_pairs(ctx).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use metaform::{bit, Predicate, Vocabulary};
    use metaform_runtime::{label, validate};

    fn collision_context() -> Context {
        // Objects 0..6; a is in the vocabulary, b is not, so 0 and 1 collide.
        let a: u64 = bit(0) | bit(1) | bit(3);
        Context {
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
        }
    }

    #[test]
    fn the_vsa_operator_proposes_something_that_validates() {
        let mut ctx = collision_context();
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, false);
        let op = VsaRetriever { dim: 2048, seed: 7, max_arity: 2 };
        assert!(op.applicable(&ctx));
        let result = op.execute(&ctx, 3);
        assert_eq!(result.status, OpStatus::Applied);
        assert!(!result.candidates.is_empty());
        let verdict = validate(&ctx, &result.candidates);
        assert!(!verdict.accepted.is_empty());
    }
}
