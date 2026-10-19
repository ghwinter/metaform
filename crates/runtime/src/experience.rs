//! Operator experience, read off the run record.
//!
//! Every controller step appends one line per offer to a record file. This module turns that file
//! into a table keyed by the state class -- object count, vocabulary size, labelled examples,
//! colliding pairs -- so the runtime can answer one question: for states like this one, which
//! operator has actually been adopting proposals, and at what cost?
//!
//! It is deliberately not a model. It is a count with a deterministic tie-break, and it only knows
//! what the record says. If a class has never been seen, it says so and the controller keeps its
//! default order.

use crate::record::{Class, Record};
use std::collections::BTreeMap;
use std::path::Path;

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

impl Experience {
    /// Reads a record file. A missing file is an empty record, not an error: the runtime has to work
    /// before it has ever run. A file whose header is not `record::VERSION` is also empty, with a
    /// warning on stderr, because guessing at another version's columns is how a table goes wrong.
    pub fn load(path: &Path) -> std::io::Result<Experience> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Experience::default()),
            Err(e) => return Err(e),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim_end_matches(['\r', '\n']) == crate::record::VERSION => {}
            _ => {
                eprintln!(
                    "! {} does not start with {}; treating it as empty",
                    path.display(),
                    crate::record::VERSION
                );
                return Ok(Experience::default());
            }
        }
        Ok(Experience::from_records(lines.filter_map(Record::parse)))
    }

    fn from_records(records: impl Iterator<Item = Record>) -> Experience {
        let mut experience = Experience::default();
        // A step's conflict flag is repeated on every offer line of that step; count it once.
        let mut counted: std::collections::BTreeSet<(u64, usize)> = std::collections::BTreeSet::new();
        for record in records {
            let tally = experience
                .table
                .entry((record.class, record.operator.clone()))
                .or_default();
            tally.attempts += 1;
            if record.adopted {
                tally.adoptions += 1;
            }
            tally.cost_sum += record.cost;
            if record.conflict && counted.insert((record.fingerprint, record.step)) {
                *experience.conflicts.entry(record.class).or_insert(0) += 1;
            }
        }
        experience
    }

    /// The operator that has adopted most often in this class, cheapest on a tie. `None` when the
    /// class has never been seen.
    pub fn suggest(&self, class: &Class) -> Option<String> {
        self.table
            .iter()
            .filter(|((c, _), _)| c == class)
            .min_by(|((_, a), ta), ((_, b), tb)| {
                tb.adoptions
                    .cmp(&ta.adoptions)
                    .then(ta.mean_cost().total_cmp(&tb.mean_cost()))
                    .then(a.cmp(b))
            })
            .map(|((_, operator), _)| operator.clone())
    }

    /// The table, sorted for printing.
    pub fn rows(&self) -> Vec<(Class, String, Tally)> {
        self.table
            .iter()
            .map(|((c, op), t)| (*c, op.clone(), t.clone()))
            .collect()
    }
}

/// The class of a state, as the record stores it.
pub fn class_of(ctx: &crate::Context) -> Class {
    Class {
        objects: ctx.objects,
        vocabulary: ctx.vocabulary.len(),
        labelled: (ctx.pos.count_ones() + ctx.neg.count_ones()) as usize,
        colliding: crate::colliding_pairs(ctx).len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(class: Class, operator: &str, adopted: bool, cost: f64, colliding: usize) -> Record {
        Record {
            fingerprint: 0x00aa,
            step: 0,
            operator: operator.to_string(),
            name: "a&b".to_string(),
            splits: 2,
            colliding,
            adopted,
            cost,
            status_before: "INSUFFICIENT".to_string(),
            status_after: "IDENTIFIED".to_string(),
            class,
            conflict: false,
        }
    }

    fn write(records: &[Record]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("metaform-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("runs-{}.tsv", COUNTER.fetch_add(1, Ordering::Relaxed)));
        let mut text = String::from(crate::record::VERSION);
        text.push('\n');
        for r in records {
            text.push_str(&r.to_line());
            text.push('\n');
        }
        std::fs::write(&path, text).unwrap();
        path
    }

    fn class_a() -> Class {
        Class { objects: 4, vocabulary: 1, labelled: 4, colliding: 2 }
    }

    fn class_b() -> Class {
        Class { objects: 6, vocabulary: 1, labelled: 4, colliding: 2 }
    }

    #[test]
    fn tallies_are_counted_per_class_and_operator() {
        let path = write(&[
            record(class_a(), "vsa-retriever", true, 0.01, 2),
            record(class_a(), "vsa-retriever", false, 0.01, 2),
        ]);
        let e = Experience::load(&path).expect("load");
        let tally = e
            .table
            .get(&(class_a(), "vsa-retriever".to_string()))
            .expect("row");
        assert_eq!(tally.attempts, 2);
        assert_eq!(tally.adoptions, 1);
        assert!((tally.adoption_rate() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_suggestion_is_the_operator_that_adopted_most() {
        let path = write(&[
            record(class_a(), "vsa-retriever", true, 0.01, 2),
            record(class_b(), "sme-analogizer", true, 49.0, 2),
            record(class_b(), "vsa-retriever", false, 0.01, 2),
        ]);
        let e = Experience::load(&path).expect("load");
        assert_eq!(e.suggest(&class_a()), Some("vsa-retriever".to_string()));
        assert_eq!(e.suggest(&class_b()), Some("sme-analogizer".to_string()));
    }

    #[test]
    fn an_unseen_class_has_no_suggestion() {
        let path = write(&[record(class_a(), "vsa-retriever", true, 0.01, 2)]);
        let e = Experience::load(&path).expect("load");
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
    fn a_file_without_the_header_is_ignored_rather_than_misread() {
        let dir = std::env::temp_dir().join(format!("metaform-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("no-header.tsv");
        std::fs::write(&path, "00aa\t0\tvsa-retriever\ta\t1\t1\t1\t0.5\tA\tB\t4\t1\t4\t0\n").unwrap();
        let e = Experience::load(&path).expect("load");
        assert!(e.table.is_empty());
    }
}
