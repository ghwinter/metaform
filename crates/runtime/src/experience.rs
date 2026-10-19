//! Operator experience, read off the run record.
//!
//! Every controller step appends a line to a record file. This module turns that file into a table
//! keyed by a coarse description of the state -- object count, vocabulary size, labelled examples,
//! colliding pairs -- so the runtime can answer one question: for states like this one, which
//! operator has actually been adopting proposals, and at what cost?
//!
//! It is deliberately not a model. It is a count with a deterministic tie-break, and it only knows
//! what the record says. If a class has never been seen, it says so and the controller keeps its
//! default order.

use std::collections::BTreeMap;
use std::path::Path;

/// A coarse class of states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Class {
    pub objects: usize,
    pub vocabulary: usize,
    pub labelled: usize,
    pub colliding: usize,
}

/// What the record says about one operator in one class.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub attempts: usize,
    pub adoptions: usize,
    pub cost_sum: f64,
}

impl Tally {
    pub fn adoption_rate(&self) -> f64 {
        if self.attempts == 0 {
            0.0
        } else {
            self.adoptions as f64 / self.attempts as f64
        }
    }

    pub fn mean_cost(&self) -> f64 {
        if self.attempts == 0 {
            0.0
        } else {
            self.cost_sum / self.attempts as f64
        }
    }
}

/// The whole table.
#[derive(Clone, Debug, Default)]
pub struct Experience {
    pub table: BTreeMap<(Class, String), Tally>,
    /// Steps whose adopted proposal could not separate everything, per class.
    pub conflicts: BTreeMap<Class, usize>,
}

/// One record line. The last three fields are optional so that records written before they existed
/// still load.
struct Line<'a> {
    operator: &'a str,
    adopted: bool,
    cost: f64,
    colliding: usize,
    objects: usize,
    vocabulary: usize,
    labelled: usize,
    conflict: bool,
    fingerprint: &'a str,
    step: &'a str,
}

fn parse(line: &str) -> Option<Line<'_>> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 10 {
        return None;
    }
    Some(Line {
        operator: f[2],
        adopted: f[6] == "1",
        cost: f[7].parse().ok()?,
        colliding: f[5].parse().ok()?,
        objects: f.get(10).and_then(|v| v.parse().ok()).unwrap_or(0),
        vocabulary: f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0),
        labelled: f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0),
        conflict: f.get(13).map(|v| *v == "1").unwrap_or(false),
        fingerprint: f[0],
        step: f[1],
    })
}

impl Experience {
    /// Reads a record file. A missing file is an empty record, not an error: the runtime has to work
    /// before it has ever run.
    pub fn load(path: &Path) -> std::io::Result<Experience> {
        let mut experience = Experience::default();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(experience),
            Err(e) => return Err(e),
        };
        // A step's conflict flag is repeated on every offer line of that step; count the step once.
        let mut counted: std::collections::BTreeSet<(String, String)> =
            std::collections::BTreeSet::new();
        for line in text.lines() {
            let Some(parsed) = parse(line) else { continue };
            let class = Class {
                objects: parsed.objects,
                vocabulary: parsed.vocabulary,
                labelled: parsed.labelled,
                colliding: parsed.colliding,
            };
            let tally = experience
                .table
                .entry((class, parsed.operator.to_string()))
                .or_default();
            tally.attempts += 1;
            if parsed.adopted {
                tally.adoptions += 1;
            }
            tally.cost_sum += parsed.cost;
            if parsed.conflict
                && counted.insert((parsed.fingerprint.to_string(), parsed.step.to_string()))
            {
                *experience.conflicts.entry(class).or_insert(0) += 1;
            }
        }
        Ok(experience)
    }

    /// The operator that has adopted most often in this class, cheapest on a tie. `None` when the
    /// class has never been seen.
    pub fn suggest(&self, class: &Class) -> Option<String> {
        let mut best: Option<(&String, &Tally)> = None;
        for ((c, operator), tally) in &self.table {
            if c != class {
                continue;
            }
            best = match best {
                None => Some((operator, tally)),
                Some((best_op, best_tally)) => {
                    let better = tally.adoptions > best_tally.adoptions
                        || (tally.adoptions == best_tally.adoptions
                            && tally.mean_cost() < best_tally.mean_cost())
                        || (tally.adoptions == best_tally.adoptions
                            && tally.mean_cost() == best_tally.mean_cost()
                            && operator < best_op);
                    if better {
                        Some((operator, tally))
                    } else {
                        Some((best_op, best_tally))
                    }
                }
            };
        }
        best.map(|(operator, _)| operator.clone())
    }

    /// The table, sorted for printing.
    pub fn rows(&self) -> Vec<(Class, String, Tally)> {
        self.table
            .iter()
            .map(|((c, op), t)| (*c, op.clone(), t.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> String {
        // Two classes; in class A the retriever adopts, in class B the analogizer does.
        [
            "00aa\t0\tvsa-retriever\ta&b\t2\t2\t1\t0.010\tINSUFFICIENT\tIDENTIFIED\t4\t1\t4",
            "00aa\t0\tvsa-retriever\ta&b\t2\t2\t0\t0.010\tINSUFFICIENT\tIDENTIFIED\t4\t1\t4",
            "00bb\t0\tsme-analogizer\tliquid\t2\t2\t1\t49.000\tINSUFFICIENT\tIDENTIFIED\t6\t1\t4",
            "00bb\t0\tvsa-retriever\tc\t0\t2\t0\t0.010\tINSUFFICIENT\tINSUFFICIENT\t6\t1\t4",
        ]
        .join("\n")
    }

    fn load(text: &str) -> Experience {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("metaform-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("runs-{}.tsv", COUNTER.fetch_add(1, Ordering::Relaxed)));
        std::fs::write(&path, text).unwrap();
        Experience::load(&path).expect("load")
    }

    #[test]
    fn tallies_are_counted_per_class_and_operator() {
        let e = load(&record());
        let class_a = Class { objects: 4, vocabulary: 1, labelled: 4, colliding: 2 };
        let tally = e.table.get(&(class_a, "vsa-retriever".to_string())).expect("row");
        assert_eq!(tally.attempts, 2);
        assert_eq!(tally.adoptions, 1);
        assert!((tally.adoption_rate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_suggestion_is_the_operator_that_adopted_most() {
        let e = load(&record());
        let class_a = Class { objects: 4, vocabulary: 1, labelled: 4, colliding: 2 };
        assert_eq!(e.suggest(&class_a), Some("vsa-retriever".to_string()));
        let class_b = Class { objects: 6, vocabulary: 1, labelled: 4, colliding: 2 };
        // In the other class the analogizer is the one that adopted.
        assert_eq!(e.suggest(&class_b), Some("sme-analogizer".to_string()));
    }

    #[test]
    fn an_unseen_class_has_no_suggestion() {
        let e = load(&record());
        let unseen = Class { objects: 12, vocabulary: 3, labelled: 6, colliding: 7 };
        assert_eq!(e.suggest(&unseen), None);
    }

    #[test]
    fn a_missing_record_is_empty() {
        let path = std::env::temp_dir().join("metaform-exp-missing.tsv");
        let _ = std::fs::remove_file(&path);
        let e = Experience::load(&path).expect("no error");
        assert!(e.table.is_empty());
    }

    #[test]
    fn old_lines_without_the_class_columns_still_load() {
        let e = load("00cc\t0\tvsa-retriever\ta\t1\t1\t1\t0.500\tINSUFFICIENT\tIDENTIFIED");
        let class = Class { objects: 0, vocabulary: 0, labelled: 0, colliding: 1 };
        assert_eq!(e.suggest(&class), Some("vsa-retriever".to_string()));
    }
}

/// The class of a state, as the record stores it.
pub fn class_of(ctx: &crate::Context) -> Class {
    let labelled = (ctx.pos.count_ones() + ctx.neg.count_ones()) as usize;
    Class {
        objects: ctx.objects,
        vocabulary: ctx.vocabulary.len(),
        labelled,
        colliding: crate::colliding_pairs(ctx).len(),
    }
}