//! Structure mapping: map one relational system onto another and propose the unmatched base
//! structure as a predicate.

use metaform::Mask;
use metaform_runtime::{Candidate, Context, OpStatus, Operator, OperatorResult};

/// An analogy component: maps one relational system onto another and turns the unmatched structure
/// of the base into proposals. The caller supplies the extensional reading of each inferred
/// relation, because only the scenario knows how a symbolic statement maps onto its objects.
pub struct SmeAnalogizer {
    pub base: metaform_sme::System,
    pub target: metaform_sme::System,
    /// For each relation name that may appear in an inference, its extension over the objects.
    pub readings: Vec<(String, Mask)>,
}

impl Operator for SmeAnalogizer {
    fn name(&self) -> &'static str {
        "sme-analogizer"
    }

    fn applicable(&self, _ctx: &Context) -> bool {
        true
    }

    fn estimate_cost(&self, _ctx: &Context) -> f32 {
        // Quadratic in the number of statements, no vectors involved.
        let statements = self.base.statements.len() + self.target.statements.len();
        (statements * statements) as f32
    }

    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult {
        let mappings = metaform_sme::map(&self.base, &self.target, 1);
        let Some(best) = mappings.first() else {
            return OperatorResult::not_applicable("the two systems share no structure");
        };
        let inferences = metaform_sme::candidate_inferences(&self.base, &self.target, best);
        let mut candidates = Vec::new();
        let mut unresolved = Vec::new();
        for statement in inferences {
            let name = statement.relation.clone();
            match self.readings.iter().find(|(n, _)| *n == name) {
                Some((_, ext)) => candidates.push(Candidate {
                    name: name.clone(),
                    extension: *ext,
                    source: self.name().to_string(),
                    detail: format!(
                        "inferred {} from the analogy (systematicity {})",
                        name, best.score.systematicity
                    ),
                    warrant: vec![format!("unmatched structure of the base, systematicity {}", best.score.systematicity)],
                    falsifier: vec!["an object that satisfies the source relation but not the mapped one".to_string()],
                    adoptable: true,
                }),
                None => unresolved.push(format!("no extensional reading for inferred relation {}", name)),
            }
            if candidates.len() >= budget {
                break;
            }
        }
        OperatorResult {
            candidates,
            evidence: vec![format!(
                "mapping: {} statements, {} entities, {} components",
                best.score.matched,
                best.entities.len(),
                best.score.components
            )],
            assumptions: vec!["the mapping is structural and says nothing about truth".to_string()],
            unresolved,
            cost: self.estimate_cost(ctx),
            status: OpStatus::Applied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::water_and_heat;
    use metaform::{bit, Predicate, Vocabulary};

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
    fn the_sme_operator_proposes_the_inferred_relation() {
        let ctx = collision_context();
        let systems = water_and_heat();
        let op = SmeAnalogizer {
            base: systems.0,
            target: systems.1,
            readings: vec![("liquid".to_string(), bit(2) | bit(5))],
        };
        assert!(op.applicable(&ctx));
        let result = op.execute(&ctx, 4);
        assert_eq!(result.status, OpStatus::Applied);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].name, "liquid");
        assert!(result.candidates[0].detail.contains("systematicity"));
    }
}
