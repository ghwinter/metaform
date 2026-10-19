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
use crate::experience::{class_of, Experience};
use metaform::Status;

/// One operator's offer during a step.
#[derive(Clone, Debug)]
pub struct Offer {
    pub operator: String,
    pub name: String,
    pub splits: usize,
    pub cost: f32,
    pub adopted: bool,
    /// False for reports that must not enter the vocabulary, such as a conflict against a label.
    pub adoptable: bool,
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
    /// True when experience named an operator that is present, so the controller put it first in
    /// the order it asks operators. This records the order that was used, not a different choice:
    /// the offers and the selection are the same as an unhinted run's.
    pub hinted: bool,
}

impl Step {
    pub fn adopted(&self) -> Option<&Offer> {
        self.offers.iter().find(|o| o.adopted)
    }
}
/// Runs the loop with the caller's operator order. Equivalent to [`run_with_experience`] with no
/// experience table.
pub fn run(ctx: &mut Context, operators: &[Box<dyn Operator>], max_steps: usize, budget: usize) -> Vec<Step> {
    run_with_experience(ctx, operators, max_steps, budget, None)
}

/// Runs the loop, using `experience` only to choose which applicable operator is asked first.
///
/// The suggestion does not add, remove or re-rank offers: every applicable operator is still asked,
/// validation still decides what is well formed, and adoption is still the adoptable offer that
/// separates the most pairs, cheapest on a tie. Each step records whether a suggestion was in force
/// in [`Step::hinted`].
pub fn run_with_experience(
    ctx: &mut Context,
    operators: &[Box<dyn Operator>],
    max_steps: usize,
    budget: usize,
    experience: Option<&Experience>,
) -> Vec<Step> {
    let mut steps = Vec::new();

    for index in 0..max_steps {
        let status_before = status_with(ctx, &[]);
        let pairs = colliding_pairs(ctx);
        if status_before != Status::Insufficient || pairs.is_empty() {
            break;
        }
        let vocabulary_before = ctx.vocabulary.len();

        // Experience only changes the order the operators are asked in: the suggested operator
        // first, everyone else in the caller's order. A suggestion for an operator the caller did
        // not supply changes nothing and does not count as a hint.
        let mut order: Vec<usize> = (0..operators.len()).collect();
        let mut hinted = false;
        if let Some(suggestion) = experience.and_then(|e| e.suggest(&class_of(ctx))) {
            if let Some(position) = operators.iter().position(|op| op.name() == suggestion) {
                let position = order.remove(position);
                order.insert(0, position);
                hinted = true;
            }
        }

        // Ask every applicable operator, validate everything it offered, and keep every offer that
        // separates at least one pair -- including the ones that will not be adopted, because the
        // record is only useful if it can see what lost.
        let mut gathered: Vec<(usize, Offer, Candidate)> = Vec::new();
        for &operator_index in &order {
            let op = &operators[operator_index];
            if !op.applicable(ctx) {
                continue;
            }
            let result = op.execute(ctx, budget);
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
                        adoptable: candidate.adoptable,
                        note: candidate.warrant.first().cloned().unwrap_or_default(),
                    };
                    gathered.push((operator_index, offer, candidate));
                }
            }
        }

        // The hint changes only the order operators are asked in, never which offer wins a tie:
        // put the offers back in the caller's operator order before the deterministic sort, so a
        // hinted run adopts exactly what an unhinted one adopts.
        gathered.sort_by_key(|(operator_index, _, _)| *operator_index);
        let mut offers: Vec<Offer> = Vec::with_capacity(gathered.len());
        let mut seen: Vec<(Offer, Candidate)> = Vec::with_capacity(gathered.len());
        for (_, offer, candidate) in gathered {
            offers.push(offer.clone());
            seen.push((offer, candidate));
        }

        // Deterministic choice: most pairs separated, then cheapest, then name.
        // Deterministic choice: adoptable offers first, then most pairs separated, then cheapest,
        // then name. A report is never adopted, whatever it separates.
        offers.sort_by(|a, b| {
            b.adoptable
                .cmp(&a.adoptable)
                .then(b.splits.cmp(&a.splits))
                .then(a.cost.total_cmp(&b.cost))
                .then(a.name.cmp(&b.name))
        });

        let conflict_notes = |offers: &[Offer]| -> Option<String> {
            let notes: Vec<String> = offers
                .iter()
                .filter(|o| !o.adoptable && !o.note.is_empty())
                .map(|o| format!("{} {}", o.name, o.note))
                .collect();
            if notes.is_empty() {
                None
            } else {
                Some(notes.join("; "))
            }
        };

        match offers.first().filter(|best| best.adoptable).cloned() {
            Some(best) => {
                if let Some((_, candidate)) = seen
                    .iter()
                    .find(|(o, _)| o.name == best.name && o.operator == best.operator)
                {
                    adopt(ctx, candidate);
                }
                let status_after = status_with(ctx, &[]);
                let mut recorded = offers.clone();
                for offer in recorded.iter_mut() {
                    offer.adopted = offer.name == best.name && offer.operator == best.operator;
                }
                // A proposal that cannot separate every pair leaves the state contradictory rather
                // than merely undecided, so anything the probe said about a doubtful label belongs
                // in the record.
                let conflict = if best.splits < pairs.len() {
                    conflict_notes(&recorded)
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
                    hinted,
                });
            }
            None => {
                // Only reports, or nothing that separates a pair: record and stop rather than loop.
                let conflict = conflict_notes(&offers);
                steps.push(Step {
                    index,
                    status_before,
                    status_after: status_before,
                    colliding_before: pairs.len(),
                    vocabulary_before,
                    offers,
                    conflict,
                    hinted,
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
    // Every byte of every field: hashing only the low byte would make two states with more than
    // eight labelled objects share a fingerprint.
    for word in [ctx.objects as u64, ctx.pos, ctx.neg] {
        for k in 0..8 {
            eat((word >> (k * 8)) as u8);
        }
    }
    for p in &ctx.vocabulary.preds {
        for b in p.name.bytes() {
            eat(b);
        }
        for k in 0..8 {
            eat((p.ext >> (k * 8)) as u8);
        }
    }
    eat(0xff);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experience::{class_of, Experience, Tally};
    use crate::label;
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

    /// A kernel-only operator: it proposes the attribute `b`, which separates the colliding
    /// pairs. No component crate is involved.
    struct ProposeB;

    impl Operator for ProposeB {
        fn name(&self) -> &'static str {
            "propose-b"
        }

        fn applicable(&self, _: &Context) -> bool {
            true
        }

        fn estimate_cost(&self, _: &Context) -> f32 {
            1.0
        }

        fn execute(&self, ctx: &Context, _: usize) -> crate::OperatorResult {
            crate::OperatorResult {
                candidates: vec![Candidate {
                    name: "b".to_string(),
                    extension: ctx.attribute("b").expect("the fixture supplies b"),
                    source: "propose-b".to_string(),
                    detail: String::new(),
                    warrant: vec!["b separates every colliding pair".to_string()],
                    falsifier: Vec::new(),
                    adoptable: true,
                }],
                evidence: Vec::new(),
                assumptions: Vec::new(),
                unresolved: Vec::new(),
                cost: 1.0,
                status: crate::OpStatus::Applied,
            }
        }
    }

    #[test]
    fn the_controller_adopts_a_separating_proposal_and_stops() {
        let mut ctx = collision();
        let operators: Vec<Box<dyn Operator>> = vec![Box::new(ProposeB)];
        let steps = run(&mut ctx, &operators, 4, 4);
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
                        adoptable: true,
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
        let steps = run(&mut ctx, &operators, 4, 4);
        assert_eq!(steps.len(), 1);
        assert!(steps[0].adopted().is_none());
        assert!(steps[0].offers.is_empty(), "nothing separated, so nothing was offered");
    }

    /// An operator that records when it is asked, so a test can see the controller's order.
    struct Ordered {
        name: &'static str,
        candidate: &'static str,
        attribute: &'static str,
        cost: f32,
        calls: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    }

    impl Operator for Ordered {
        fn name(&self) -> &'static str {
            self.name
        }

        fn applicable(&self, _: &Context) -> bool {
            true
        }

        fn estimate_cost(&self, _: &Context) -> f32 {
            self.cost
        }

        fn execute(&self, ctx: &Context, _: usize) -> crate::OperatorResult {
            self.calls.borrow_mut().push(self.name);
            crate::OperatorResult {
                candidates: vec![Candidate {
                    name: self.candidate.to_string(),
                    extension: ctx.attribute(self.attribute).expect("the fixture supplies the attribute"),
                    source: self.name.to_string(),
                    detail: String::new(),
                    warrant: vec![format!("{} always offers {}", self.name, self.candidate)],
                    falsifier: Vec::new(),
                    adoptable: true,
                }],
                evidence: Vec::new(),
                assumptions: Vec::new(),
                unresolved: Vec::new(),
                cost: self.cost,
                status: crate::OpStatus::Applied,
            }
        }
    }

    fn collision_with_weak_d() -> Context {
        // `d` separates only one of the two colliding pairs, and is cheaper than `b`.
        let mut ctx = collision();
        ctx.attributes.push(("d".to_string(), bit(0) | bit(2)));
        ctx
    }

    fn ordered_operators(
        calls: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    ) -> Vec<Box<dyn Operator>> {
        vec![
            Box::new(Ordered {
                name: "first",
                candidate: "b",
                attribute: "b",
                cost: 1.0,
                calls: calls.clone(),
            }),
            Box::new(Ordered {
                name: "second",
                candidate: "d",
                attribute: "d",
                cost: 0.1,
                calls,
            }),
        ]
    }

    #[test]
    fn experience_offers_the_suggested_operator_first_without_changing_the_choice() {
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let operators = ordered_operators(calls.clone());

        let mut default_ctx = collision_with_weak_d();
        let default_steps = run(&mut default_ctx, &operators, 4, 4);
        assert_eq!(
            *calls.borrow(),
            vec!["first", "second"],
            "without experience the caller's order stands"
        );

        calls.borrow_mut().clear();

        let class = class_of(&collision_with_weak_d());
        let experience = Experience::from_rows(vec![(
            class,
            "second".to_string(),
            Tally { attempts: 1, adoptions: 1, cost_sum: 0.1 },
        )]);
        let mut hinted_ctx = collision_with_weak_d();
        let hinted_steps =
            run_with_experience(&mut hinted_ctx, &operators, 4, 4, Some(&experience));
        assert_eq!(
            *calls.borrow(),
            vec!["second", "first"],
            "the suggested operator is asked first"
        );

        assert!(hinted_steps[0].hinted, "the step records that a hint was in force");
        assert!(!default_steps[0].hinted, "an unhinted run does not claim a hint");

        let adopted = |steps: &[Step]| {
            steps[0].adopted().expect("an offer was adopted").operator.clone()
        };
        assert_eq!(adopted(&default_steps), "first");
        assert_eq!(
            adopted(&hinted_steps),
            "first",
            "the hint must not override the pairs-separated rule"
        );
        assert_eq!(default_ctx.vocabulary.len(), hinted_ctx.vocabulary.len());
        assert_eq!(status_with(&default_ctx, &[]), status_with(&hinted_ctx, &[]));
    }

    #[test]
    fn a_hint_does_not_break_a_complete_tie() {
        // Both operators offer the same predicate at the same cost, so the existing stable sort
        // decides by caller order. Asking the suggested operator first must not flip that.
        let calls = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let operators: Vec<Box<dyn Operator>> = vec![
            Box::new(Ordered {
                name: "first",
                candidate: "b",
                attribute: "b",
                cost: 1.0,
                calls: calls.clone(),
            }),
            Box::new(Ordered {
                name: "second",
                candidate: "b",
                attribute: "b",
                cost: 1.0,
                calls: calls.clone(),
            }),
        ];
        let class = class_of(&collision());
        let experience = Experience::from_rows(vec![(
            class,
            "second".to_string(),
            Tally { attempts: 1, adoptions: 1, cost_sum: 1.0 },
        )]);

        let mut default_ctx = collision();
        let default_steps = run(&mut default_ctx, &operators, 4, 4);
        calls.borrow_mut().clear();
        let mut hinted_ctx = collision();
        let hinted_steps =
            run_with_experience(&mut hinted_ctx, &operators, 4, 4, Some(&experience));

        assert_eq!(*calls.borrow(), vec!["second", "first"]);
        let adopted = |steps: &[Step]| {
            steps[0].adopted().expect("an offer was adopted").operator.clone()
        };
        assert_eq!(adopted(&default_steps), "first");
        assert_eq!(
            adopted(&hinted_steps),
            "first",
            "a complete tie must keep resolving in caller order"
        );
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
