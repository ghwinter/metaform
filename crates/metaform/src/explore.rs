//! Attribute exploration over the state's predicate context, ported from conexp-clj's
//! `explore-attributes` (`src/main/clojure/conexp/fca/exploration.clj`, line 354) to u64 bitmasks.
//!
//! The reference explores a formal context with an interactive expert: it walks the closed sets of
//! the law theory with Next-Closure, asks the expert whether `A -> A''` holds in the real world, and
//! when the answer is no adds the counterexample object to the context. That new object is what
//! makes later questions smaller. Our port cannot add objects (the state is read-only), so it uses
//! the evidence already in the state: the objects with a confirmed label under `theta`. The
//! implication is confirmed when the evidence agrees and no object contradicts it, refuted when an
//! object contradicts it, and left open when no evidence object matches the premise. A refutation
//! therefore shrinks nothing; only an added law does.
//!
//! Two further deviations are deliberate:
//!
//! - The closed sets are enumerated in `(cardinality, mask)` order by a priority-queue search over
//!   closures (each closed set is reached from a closed subset), not in the reference's lectic order.
//!   The order is total and independent of hash iteration, so the same state gives the same bytes.
//! - Enumeration is lazy: the budget stops the search after `max_questions` premises, and the queue
//!   only holds the boundary of the visited closed sets, never all `2^p` subsets.
//!
//! The theory can be seeded from a laws file (`Context::run_with_theory_and_refutations`): the walk
//! then enumerates the closed sets of the seed laws plus every law confirmed in the run. A premise a
//! seed law decides is never examined. A recorded refutation is not a law: it never enters the
//! closure, but the premise it names is treated as decided (no question, no implication) and appears
//! in `refuted` with the source that recorded it.
//!
//! A law stores its conclusion disjoint from its premise (`B' \ B`), the convention of the
//! reference's `make-implication`; the closure test is unchanged because the premise is contained
//! in the set being closed.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

use serde::Serialize;

use crate::suggest::status_str;
use crate::{Explorer, Status};

/// Default number of premises examined before the command reports `"stopped":"budget"`.
pub const DEFAULT_BUDGET: usize = 1000;

/// The context the procedure runs on: one bit per predicate, one bit per object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// Predicate names, in the order that assigns predicate bits. The CLI passes them name-sorted.
    pub names: Vec<String>,
    /// Extension of each predicate as a mask over the objects.
    pub ext: Vec<u64>,
    /// Intent of each object as a mask over the predicates.
    pub intents: Vec<u64>,
    /// Object count (`n <= 64`).
    pub n: usize,
    /// Objects whose label is confirmed under `theta`: the read-only evidence oracle.
    pub evidence: u64,
}

impl Context {
    /// Build a context from columns. Extensions are clipped to the `n` objects; each object's intent
    /// is computed from those columns.
    pub fn new(names: Vec<String>, ext: Vec<u64>, n: usize, evidence: u64) -> Result<Self, String> {
        if n > 64 {
            return Err(format!("explore supports at most 64 objects (got {n})"));
        }
        if names.len() != ext.len() {
            return Err("predicate names and extensions have different lengths".into());
        }
        if names.len() > 64 {
            return Err(format!(
                "explore supports at most 64 predicates (got {})",
                names.len()
            ));
        }
        let objects = object_mask(n);
        let ext: Vec<u64> = ext.iter().map(|m| m & objects).collect();
        let mut intents = vec![0u64; n];
        for (p, &m) in ext.iter().enumerate() {
            let mut bits = m;
            while bits != 0 {
                let x = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                intents[x] |= 1u64 << p;
            }
        }
        Ok(Self {
            names,
            ext,
            intents,
            n,
            evidence: evidence & objects,
        })
    }

    /// Read the vocabulary and object count from an [`Explorer`], sorting predicates by name so the
    /// bit order (and therefore every emitted mask order) does not depend on `HashMap` iteration.
    /// Duplicate names keep their state order; `sort_by` is stable.
    pub fn from_explorer(e: &Explorer, evidence: u64) -> Result<Self, String> {
        let mut preds: Vec<(String, u64)> = e
            .vocab
            .preds
            .iter()
            .map(|p| (p.name.clone(), p.ext))
            .collect();
        preds.sort_by(|a, b| a.0.cmp(&b.0));
        let names = preds.iter().map(|(name, _)| name.clone()).collect();
        let ext = preds.iter().map(|(_, m)| *m).collect();
        Self::new(names, ext, e.n, evidence)
    }

    /// Mask of the first `n` objects.
    pub fn objects(&self) -> u64 {
        object_mask(self.n)
    }

    /// Mask of all predicates.
    pub fn top(&self) -> u64 {
        predicate_mask(self.names.len())
    }

    /// `extent(A)`: the objects that have every predicate in `A`. The empty premise has extent `X`.
    pub fn extent(&self, premise: u64) -> u64 {
        let mut out = self.objects();
        let mut bits = premise;
        while bits != 0 {
            let p = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            out &= self.ext[p];
        }
        out
    }

    /// `intent(O)`: the predicates shared by every object in `O`. The empty set has intent `M`
    /// (all predicates), the top of the predicate lattice.
    pub fn intent(&self, objects: u64) -> u64 {
        let mut out = self.top();
        let mut bits = objects;
        while bits != 0 {
            let x = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            out &= self.intents[x];
        }
        out
    }

    /// Names of a predicate mask, sorted. The CLI's bit order is already name-sorted; sorting here
    /// keeps the guarantee even for a context built directly by a caller or test.
    pub fn names_of(&self, mask: u64) -> Vec<String> {
        let mut out = Vec::new();
        let mut bits = mask;
        while bits != 0 {
            let p = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            out.push(self.names[p].clone());
        }
        out.sort();
        out
    }

    /// Run the exploration, examining at most `max_questions` closed-set premises.
    ///
    /// `status` is the state's own status under the same `theta` (`state need`), reported in the
    /// `status` field so this command cannot disagree with the rest of the CLI.
    pub fn run(&self, max_questions: usize, status: Status) -> Report {
        self.run_with_theory(max_questions, status, &[])
    }

    /// Run the exploration with an initial law theory, expressed as `(premise, conclusion)` masks.
    ///
    /// The walk enumerates the closed sets of `theory` plus every law confirmed during this run, so
    /// a premise that a law already decides is never examined (and therefore never asked again).
    /// The report still lists only the implications confirmed in this run, matching the reference,
    /// whose return value subtracts the background knowledge.
    pub fn run_with_theory(
        &self,
        max_questions: usize,
        status: Status,
        theory: &[(u64, u64)],
    ) -> Report {
        self.run_with_theory_and_refutations(max_questions, status, theory, &[])
    }

    /// Run the exploration with both a seed theory and recorded refutations.
    ///
    /// A refutation is not a law: it is never put into `laws`, so it cannot make the closure believe
    /// the implication it denies. Instead, when the walk reaches a premise with a recorded
    /// refutation, that premise is treated as decided: it produces no `open` question and no
    /// `implication`, and the refutation is reported in `refuted` with its recorded source (and,
    /// when the note named one, its counterexample object).
    pub fn run_with_theory_and_refutations(
        &self,
        max_questions: usize,
        status: Status,
        theory: &[(u64, u64)],
        refutations: &[Refutation],
    ) -> Report {
        let mut laws: Vec<(u64, u64)> = theory.to_vec();
        let mut implications: Vec<Implication> = Vec::new();
        let mut refuted: Vec<Refuted> = Vec::new();
        let mut open: Vec<Open> = Vec::new();
        let mut examined: HashSet<u64> = HashSet::new();
        let mut closed_sets = 0usize;
        let mut stopped = false;
        let mut walk = ClosedSets::new(laws.clone(), self.top());

        loop {
            if closed_sets == max_questions {
                // There is always at least the top closed set, so this lookahead is what separates
                // "finished exactly at the budget" from "there was more to examine".
                if walk.next(&examined).is_some() {
                    stopped = true;
                }
                break;
            }
            let Some(premise) = walk.next(&examined) else {
                break;
            };
            examined.insert(premise);
            closed_sets += 1;

            let recorded: Vec<&Refutation> = refutations
                .iter()
                .filter(|refutation| refutation.premise == premise)
                .collect();
            if !recorded.is_empty() {
                // The expert already decided this premise. Do not ask about it and do not derive an
                // implication from it; just report the record, with the source that made it.
                for refutation in recorded {
                    refuted.push(Refuted {
                        premise: self.names_of(premise),
                        conclusion: self.names_of(refutation.conclusion),
                        counterexample: refutation.counterexample,
                        by: refutation.source.clone(),
                    });
                }
                continue;
            }

            let evidence_objects = self.extent(premise) & self.evidence;
            if evidence_objects == 0 {
                // No evidence object matches the premise: the evidence cannot decide. The vacuous
                // evidence closure is the top, so the question asks whether the premise can occur.
                let conclusion = self.top() & !premise;
                if conclusion != 0 {
                    open.push(Open {
                        premise: self.names_of(premise),
                        conclusion: self.names_of(conclusion),
                    });
                }
                continue;
            }

            let closure = self.intent(evidence_objects);
            let conclusion = closure & !premise;
            if conclusion == 0 {
                // The implication holds in all evidence and adds nothing; no question.
                continue;
            }

            match self.counterexample(premise, conclusion) {
                Some(id) => refuted.push(Refuted {
                    premise: self.names_of(premise),
                    conclusion: self.names_of(conclusion),
                    counterexample: Some(id),
                    by: "evidence".to_string(),
                }),
                None => {
                    // The evidence suggests the implication and no object contradicts it. Recording
                    // the law is the port's substitute for adding the reference's counterexample
                    // object: only this branch can shrink later questions.
                    implications.push(Implication {
                        premise: self.names_of(premise),
                        conclusion: self.names_of(conclusion),
                        by: "evidence",
                    });
                    laws.push((premise, conclusion));
                    walk = ClosedSets::new(laws.clone(), self.top());
                }
            }
        }

        Report {
            status: status_str(status).to_string(),
            questions: open.len(),
            budget: max_questions,
            implications,
            refuted,
            open,
            closed_sets,
            stopped: stopped.then_some("budget"),
        }
    }

    /// First object in `extent(premise)`, in ascending id order, whose intent lacks at least one
    /// attribute of `conclusion`. Evidence objects cannot qualify: `conclusion` is derived from
    /// their intersection, so a counterexample is always an unlabelled (or weakly labelled) object.
    fn counterexample(&self, premise: u64, conclusion: u64) -> Option<usize> {
        let mut objects = self.extent(premise);
        while objects != 0 {
            let x = objects.trailing_zeros() as usize;
            objects &= objects - 1;
            if self.intents[x] & conclusion != conclusion {
                return Some(x);
            }
        }
        None
    }
}

/// One confirmed implication. `conclusion` is disjoint from `premise`, as in the reference's
/// `make-implication`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Implication {
    pub premise: Vec<String>,
    pub conclusion: Vec<String>,
    pub by: &'static str,
}

/// One implication the evidence refuted or the expert recorded as refuted, with the object that
/// violates it when one is known.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Refuted {
    pub premise: Vec<String>,
    pub conclusion: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<usize>,
    /// `evidence` for a refutation found in this run, or the recorded source (`expert`).
    pub by: String,
}

/// One recorded refutation, with masks resolved against the context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refutation {
    pub premise: u64,
    pub conclusion: u64,
    pub counterexample: Option<usize>,
    pub source: String,
}

/// One implication the evidence cannot decide.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Open {
    pub premise: Vec<String>,
    pub conclusion: Vec<String>,
}

/// The command's whole answer. Field order is the JSON order and is stable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    pub status: String,
    /// Questions the evidence could not answer (`open.len()`).
    pub questions: usize,
    /// The `--max-questions` cap on premises examined.
    pub budget: usize,
    pub implications: Vec<Implication>,
    pub refuted: Vec<Refuted>,
    pub open: Vec<Open>,
    /// Closed-set premises examined. Rebuilding the walk after a law does not re-count a premise.
    pub closed_sets: usize,
    /// Present only when the budget stopped a search that still had work left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped: Option<&'static str>,
}

/// Render a report as one line of JSON.
pub fn to_json(report: &Report) -> String {
    serde_json::to_string(report).expect("explore report serializes")
}

/// One `open` question with the stable id [`questions_json`] assigns it: its index in `open`
/// order, which is the deterministic `(cardinality, mask)` walk order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Question {
    pub id: usize,
    pub premise: Vec<String>,
    pub conclusion: Vec<String>,
    pub text: String,
}

/// The `--emit-questions` file. The answer command reads exactly this shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Questions {
    pub status: String,
    pub questions: Vec<Question>,
}

impl Report {
    /// The `open` questions, numbered in report order.
    pub fn questions(&self) -> Vec<Question> {
        self.open
            .iter()
            .enumerate()
            .map(|(id, open)| Question {
                id,
                premise: open.premise.clone(),
                conclusion: open.conclusion.clone(),
                text: question_text(&open.premise, &open.conclusion),
            })
            .collect()
    }
}

/// Render the open questions as the one-line JSON the answer command consumes.
pub fn questions_json(report: &Report) -> String {
    let questions = Questions {
        status: report.status.clone(),
        questions: report.questions(),
    };
    serde_json::to_string(&questions).expect("questions serialize")
}

/// The question the reference asks in prose ("Does the implication A -> B hold?"), in the form the
/// exploration output uses. Empty sides get a word so the sentence stays readable.
fn question_text(premise: &[String], conclusion: &[String]) -> String {
    let antecedent = if premise.is_empty() {
        "no attributes".to_string()
    } else {
        premise.join(", ")
    };
    let consequent = if conclusion.is_empty() {
        "nothing".to_string()
    } else {
        conclusion.join(", ")
    };
    format!("Does every object with {antecedent} also have {consequent}?")
}

/// Deterministic lazy enumeration of the closed sets of an implication theory, in
/// `(cardinality, mask)` order.
///
/// The next closed set is found by a priority-queue search: every closed set is reachable from a
/// closed proper subset by adding one predicate and closing again, and that step strictly raises
/// the cardinality. Popping the smallest `(cardinality, mask)` therefore visits the fixed points in
/// exactly the requested order, while only the visited boundary is held in memory.
struct ClosedSets {
    laws: Vec<(u64, u64)>,
    top: u64,
    heap: BinaryHeap<Reverse<(u32, u64)>>,
    seen: HashSet<u64>,
}

impl ClosedSets {
    fn new(laws: Vec<(u64, u64)>, top: u64) -> Self {
        let least = close(0, &laws);
        let mut seen = HashSet::new();
        seen.insert(least);
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((least.count_ones(), least)));
        Self {
            laws,
            top,
            heap,
            seen,
        }
    }

    /// Next closed set that has not been examined. Examined sets are still expanded: a path to a
    /// later fixed point can pass through one.
    fn next(&mut self, examined: &HashSet<u64>) -> Option<u64> {
        while let Some(Reverse((_, set))) = self.heap.pop() {
            let mut free = self.top & !set;
            while free != 0 {
                let bit = free & free.wrapping_neg();
                free ^= bit;
                let next = close(set | bit, &self.laws);
                if self.seen.insert(next) {
                    self.heap.push(Reverse((next.count_ones(), next)));
                }
            }
            if !examined.contains(&set) {
                return Some(set);
            }
        }
        None
    }
}

/// Downing-Gallier-style closure under a list of implications, iterated to a fixed point. The list
/// is small (one law per confirmed implication) and its order is the deterministic discovery order.
fn close(mut set: u64, laws: &[(u64, u64)]) -> u64 {
    loop {
        let mut changed = false;
        for &(premise, conclusion) in laws {
            if set & premise == premise {
                let next = set | conclusion;
                if next != set {
                    set = next;
                    changed = true;
                }
            }
        }
        if !changed {
            return set;
        }
    }
}

/// Mask of the first `n` objects.
fn object_mask(n: usize) -> u64 {
    if n >= 64 {
        u64::MAX
    } else if n == 0 {
        0
    } else {
        (1u64 << n) - 1
    }
}

/// Mask of `p` predicates.
fn predicate_mask(p: usize) -> u64 {
    if p >= 64 {
        u64::MAX
    } else if p == 0 {
        0
    } else {
        (1u64 << p) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(names: &[&str], ext: &[&[usize]], n: usize, evidence: &[usize]) -> Context {
        let names = names.iter().map(|s| s.to_string()).collect();
        let ext = ext
            .iter()
            .map(|ids| ids.iter().fold(0u64, |m, &x| m | (1u64 << x)))
            .collect();
        let evidence = evidence.iter().fold(0u64, |m, &x| m | (1u64 << x));
        Context::new(names, ext, n, evidence).unwrap()
    }

    #[test]
    fn closure_is_the_least_fixpoint() {
        let a = 0b001;
        let b = 0b010;
        let laws = vec![(a, b)];
        assert_eq!(close(0, &laws), 0);
        assert_eq!(close(a, &laws), a | b);
        assert_eq!(close(b, &laws), b);
        assert_eq!(close(a | b, &laws), a | b);
    }

    #[test]
    fn closed_sets_come_in_cardinality_then_mask_order() {
        let mut walk = ClosedSets::new(Vec::new(), 0b111);
        let examined = HashSet::new();
        let mut got = Vec::new();
        while let Some(set) = walk.next(&examined) {
            got.push(set);
        }
        assert_eq!(got, vec![0, 1, 2, 4, 3, 5, 6, 7]);
    }

    #[test]
    fn closed_sets_under_a_law_keep_the_order() {
        // a -> b: every set containing a must also contain b. The closed sets in
        // (cardinality, mask) order are {}, {b}, {c}, {a,b}, {b,c}, {a,b,c}.
        let mut walk = ClosedSets::new(vec![(0b001, 0b010)], 0b111);
        let examined = HashSet::new();
        let mut got = Vec::new();
        while let Some(set) = walk.next(&examined) {
            got.push(set);
        }
        assert_eq!(got, vec![0, 2, 4, 3, 6, 7]);
    }

    #[test]
    fn implication_confirmed_by_evidence_is_disjoint_and_recorded() {
        // a = {0}, b = {0,1}: all three objects are evidence, so a -> b is suggested and object 0,
        // the only object with a, has b. No object can refute it.
        let c = ctx(&["a", "b"], &[&[0], &[0, 1]], 3, &[0, 1, 2]);
        let report = c.run(100, Status::Identified);
        assert_eq!(report.implications.len(), 1, "{report:?}");
        assert_eq!(report.implications[0].premise, vec!["a"]);
        assert_eq!(report.implications[0].conclusion, vec!["b"]);
        assert_eq!(report.implications[0].by, "evidence");
        assert!(report.refuted.is_empty());
        assert!(report.open.is_empty());
        assert_eq!(report.stopped, None);
    }

    #[test]
    fn refutation_names_the_violating_object() {
        // a = {0,2}, b = {0}; evidence is {0,1}. Object 0 says a -> b, object 2 has a but not b.
        let c = ctx(&["a", "b"], &[&[0, 2], &[0]], 3, &[0, 1]);
        let report = c.run(100, Status::Undetermined);
        let r = report
            .refuted
            .iter()
            .find(|r| r.premise == vec!["a"])
            .expect("a -> b must be refuted");
        assert_eq!(r.conclusion, vec!["b"]);
        assert_eq!(r.counterexample, Some(2));
        assert!(report.stopped.is_none());
    }

    #[test]
    fn no_matching_evidence_leaves_open_questions() {
        let c = ctx(&["a", "b"], &[&[0], &[1]], 2, &[]);
        let report = c.run(100, Status::Undetermined);
        assert_eq!(report.questions, 3, "{report:?}");
        assert!(report.open.iter().any(|q| q.premise == vec!["a"]));
        assert!(report.open.iter().any(|q| q.premise == vec!["b"]));
        assert!(report.open.iter().any(|q| q.premise.is_empty()));
    }

    #[test]
    fn budget_stops_a_large_law_free_family_without_materialising_it() {
        // Eight predicates with no evidence: all 256 subsets are closed, all are open questions.
        // Fifty premises must be enough to stop, not to enumerate 256.
        let names: Vec<&str> = ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7"].to_vec();
        let ext: Vec<&[usize]> = vec![&[], &[], &[], &[], &[], &[], &[], &[]];
        let c = ctx(&names, &ext, 1, &[]);
        let report = c.run(50, Status::Undetermined);
        assert_eq!(report.stopped, Some("budget"));
        assert_eq!(report.closed_sets, 50);
        assert_eq!(report.questions, 50);
        assert!(report.implications.is_empty());
        assert!(
            report.closed_sets < 256,
            "the full family must not be materialised"
        );
    }

    #[test]
    fn zero_budget_stops_before_any_premise() {
        let c = ctx(&["a"], &[&[0]], 1, &[0]);
        let report = c.run(0, Status::Identified);
        assert_eq!(report.stopped, Some("budget"));
        assert_eq!(report.closed_sets, 0);
    }

    #[test]
    fn same_context_gives_the_same_bytes() {
        let c = ctx(&["a", "b", "c"], &[&[0, 1], &[0], &[1]], 3, &[0, 1]);
        let one = to_json(&c.run(20, Status::Undetermined));
        let two = to_json(&c.run(20, Status::Undetermined));
        assert_eq!(one, two);
    }

    #[test]
    fn names_are_sorted_in_every_entry() {
        let c = ctx(&["zeta", "alpha", "m"], &[&[0], &[0], &[0]], 1, &[0]);
        let report = c.run(100, Status::Identified);
        for q in &report.open {
            let mut sorted = q.conclusion.clone();
            sorted.sort();
            assert_eq!(q.conclusion, sorted);
        }
        for i in &report.implications {
            let mut sorted = i.conclusion.clone();
            sorted.sort();
            assert_eq!(i.conclusion, sorted);
        }
    }
}
