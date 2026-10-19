//! A proposer that also says which label it had to disbelieve.
//!
//! The controller decides by pairs separated, and that count is computed against the labels it was
//! given. When one label is wrong, a concept that fits the true structure separates fewer pairs than
//! a concept that fits the mistake, so the honest proposal loses and nothing records why. This
//! operator looks for exactly that situation: a predicate that separates every colliding pair except
//! the ones touching a single labelled object. It offers that predicate and says which object it had
//! to disbelieve.
//!
//! It cannot correct the label -- correcting evidence is not something a proposal may do -- but it
//! can make the conflict visible, which is what the kernel's probe does for inherited classes.
//!
//! The search is exhaustive over conjunctions of at most `max_arity` attributes, so this is a
//! mechanical proposer, not a clever one. Its value is the second half of what it returns.

use crate::{colliding_pairs, Context, OpStatus, Operator, OperatorResult};
use metaform::Mask;

/// Exhaustive conjunctions of the attributes, with readable names.
fn conjunctions(attributes: &[(String, Mask)], max_arity: usize) -> Vec<(String, Mask)> {
    let mut out = Vec::new();
    let n = attributes.len();
    let arity = max_arity.clamp(1, n.max(1));
    for size in 1..=arity {
        let mut idx: Vec<usize> = (0..size).collect();
        loop {
            let mut name = attributes[idx[0]].0.clone();
            let mut mask = attributes[idx[0]].1;
            for &i in &idx[1..] {
                name.push('&');
                name.push_str(&attributes[i].0);
                mask &= attributes[i].1;
            }
            out.push((name, mask));
            let mut i = size;
            let mut done = false;
            loop {
                if i == 0 {
                    done = true;
                    break;
                }
                i -= 1;
                if idx[i] != i + n - size {
                    idx[i] += 1;
                    for j in (i + 1)..size {
                        idx[j] = idx[j - 1] + 1;
                    }
                    break;
                }
            }
            if done {
                break;
            }
        }
    }
    out
}

/// Looks for a concept that would separate every colliding pair if one label were wrong.
#[derive(Clone, Debug)]
pub struct ProbeLabels {
    pub max_arity: usize,
}

impl Default for ProbeLabels {
    fn default() -> Self {
        ProbeLabels { max_arity: 2 }
    }
}

impl ProbeLabels {
    pub fn new(max_arity: usize) -> Self {
        ProbeLabels { max_arity }
    }
}

impl Operator for ProbeLabels {
    fn name(&self) -> &'static str {
        "probe-labels"
    }

    fn applicable(&self, ctx: &Context) -> bool {
        !colliding_pairs(ctx).is_empty()
    }

    fn estimate_cost(&self, _ctx: &Context) -> f32 {
        // One exhaustive sweep over the attribute conjunctions.
        2.0
    }

    fn execute(&self, ctx: &Context, budget: usize) -> OperatorResult {
        let pairs = colliding_pairs(ctx);
        // The direct question: for each labelled object, if that one label were wrong, would some
        // concept separate everything? Enumerating suspects first is what makes the answer name the
        // right object; enumerating concepts first ranks by compactness and can miss it entirely.
        // One entry per suspect: the most compact concept that becomes available if that label is
        // reconsidered. Grouping by concept instead would let one suspect crowd the others out of
        // the budget, and the point of the operator is to name suspects.
        let mut by_suspect: std::collections::BTreeMap<usize, (usize, String, Mask)> =
            std::collections::BTreeMap::new();
        let mut checked = 0usize;

        for suspect in 0..ctx.objects {
            let labelled = (ctx.pos >> suspect) & 1 == 1 || (ctx.neg >> suspect) & 1 == 1;
            if !labelled {
                continue;
            }
            checked += 1;
            let mut flipped = ctx.clone();
            if (ctx.pos >> suspect) & 1 == 1 {
                flipped.pos &= !(1u64 << suspect);
                flipped.neg |= 1u64 << suspect;
            } else {
                flipped.neg &= !(1u64 << suspect);
                flipped.pos |= 1u64 << suspect;
            }
            let after = colliding_pairs(&flipped);
            // Flipping the only positive would leave nothing to separate, and then every concept
            // "explains" the state vacuously. That is not a repair, so it is not offered.
            if after.is_empty() {
                continue;
            }
            for (name, mask) in conjunctions(&ctx.attributes, self.max_arity) {
                if !after.iter().all(|&(p, n)| ((mask >> p) & 1) != ((mask >> n) & 1)) {
                    continue;
                }
                let size = mask.count_ones() as usize;
                let better = match by_suspect.get(&suspect) {
                    None => true,
                    Some((best_size, _, _)) => size < *best_size,
                };
                if better {
                    by_suspect.insert(suspect, (size, name, mask));
                }
            }
        }

        if by_suspect.is_empty() {
            return OperatorResult::not_applicable("no single label explains what is left");
        }

        let mut rows: Vec<(usize, usize, String, Mask)> = by_suspect
            .into_iter()
            .map(|(suspect, (size, name, mask))| (size, suspect, name, mask))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut candidates = Vec::new();
        let mut unresolved = Vec::new();
        for (_, suspect, name, mask) in rows.iter().take(budget) {
            let splits = pairs
                .iter()
                .filter(|&&(p, n)| ((mask >> p) & 1) != ((mask >> n) & 1))
                .count();
            candidates.push(crate::Candidate {
                name: format!("{} (if label {} is wrong)", name, suspect),
                extension: *mask,
                source: "probe-labels".to_string(),
                detail: format!(
                    "separates {} of {} pairs as labelled, all of them if object {} is re-examined",
                    splits,
                    pairs.len(),
                    suspect
                ),
                warrant: vec![format!("flipping object {} admits this concept", suspect)],
                falsifier: vec![format!("object {} is labelled correctly", suspect)],
                adoptable: true,
            });
            unresolved.push(format!(
                "label of object {}: flipping it admits a concept that separates everything",
                suspect
            ));
        }

        OperatorResult {
            candidates,
            evidence: vec![format!(
                "{} labelled object(s) examined against {} colliding pair(s)",
                checked,
                pairs.len()
            )],
            assumptions: Vec::new(),
            unresolved,
            cost: 2.0,
            status: OpStatus::Applied,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{context, label, Context};
    use metaform::{bit, Predicate, Vocabulary};

    /// The vocabulary sees nothing; b holds for 0 and 1, while 2 is labelled positive by mistake.
    fn noisy() -> Context {
        let a = bit(0) | bit(1) | bit(2) | bit(3);
        let b = bit(0) | bit(1);
        let mut ctx: Context = context(
            4,
            Vocabulary::with(vec![Predicate::new("a", a)]),
            vec![("a".to_string(), a), ("b".to_string(), b)],
        );
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, true);
        label(&mut ctx, 2, true);
        label(&mut ctx, 3, false);
        ctx
    }

    #[test]
    fn a_wrong_label_is_named_not_obeyed() {
        let ctx = noisy();
        let probe = ProbeLabels::new(2);
        assert!(probe.applicable(&ctx));
        let result = probe.execute(&ctx, 4);
        assert!(!result.candidates.is_empty(), "the conflict should be offered");
        assert!(
            result.candidates.iter().any(|c| c.name.contains("if label 2 is wrong")),
            "{:?}",
            result.candidates.iter().map(|c| c.name.clone()).collect::<Vec<_>>()
        );
        assert!(result.unresolved.iter().any(|u| u.contains("object 2")));
    }

    #[test]
    fn nothing_is_offered_when_no_single_label_explains_the_rest() {
        // Two leftover pairs with disjoint endpoints: no single object can take the blame.
        let a = bit(0) | bit(1) | bit(2) | bit(3);
        let b = bit(0) | bit(2);
        let c = bit(1) | bit(3);
        let mut ctx: Context = context(
            4,
            Vocabulary::with(vec![Predicate::new("a", a)]),
            vec![
                ("a".to_string(), a),
                ("b".to_string(), b),
                ("c".to_string(), c),
            ],
        );
        label(&mut ctx, 0, true);
        label(&mut ctx, 1, true);
        label(&mut ctx, 2, false);
        label(&mut ctx, 3, false);
        let result = ProbeLabels::new(2).execute(&ctx, 4);
        assert!(result.candidates.is_empty(), "{:?}", result.candidates.iter().map(|c| c.name.clone()).collect::<Vec<_>>());
    }
}
