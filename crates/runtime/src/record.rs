//! The run record: one tab-separated line per offer, written and read by the same type.
//!
//! The controller writes a line for every offer it saw, adopted or not, because a record that
//! only keeps the winner cannot answer why the winner won. A file starts with a version header;
//! the parser rejects a file whose header it does not know rather than guessing.
//!
//! The columns are positional and fixed, so the format lives here and nowhere else. Adding a
//! column means bumping `VERSION`.

/// The header line that opens a record file.
pub const VERSION: &str = "# metaform-run-record/1";

/// A coarse description of a state, used to group runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Class {
    pub objects: usize,
    pub vocabulary: usize,
    pub labelled: usize,
    pub colliding: usize,
}

/// One offer as it was recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// Fingerprint of the state the offer was made against.
    pub fingerprint: u64,
    pub step: usize,
    pub operator: String,
    pub name: String,
    /// Colliding pairs the proposal separates, under the labels as given.
    pub splits: usize,
    /// Colliding pairs in the state before the step.
    pub colliding: usize,
    pub adopted: bool,
    pub cost: f64,
    pub status_before: String,
    pub status_after: String,
    pub class: Class,
    /// True when this step's adopted proposal could not separate every pair.
    pub conflict: bool,
}

impl Record {
    /// The line to write. Names may contain spaces; they may not contain tabs.
    pub fn to_line(&self) -> String {
        [
            format!("{:016x}", self.fingerprint),
            self.step.to_string(),
            self.operator.clone(),
            self.name.replace('\t', " "),
            self.splits.to_string(),
            self.colliding.to_string(),
            u8::from(self.adopted).to_string(),
            format!("{:.3}", self.cost),
            self.status_before.clone(),
            self.status_after.clone(),
            self.class.objects.to_string(),
            self.class.vocabulary.to_string(),
            self.class.labelled.to_string(),
            u8::from(self.conflict).to_string(),
        ]
        .join("\t")
    }

    /// Parses one line. Returns `None` for the header, comments, blank lines and anything that does
    /// not have the current number of columns.
    pub fn parse(line: &str) -> Option<Record> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 14 {
            return None;
        }
        Some(Record {
            fingerprint: u64::from_str_radix(f[0], 16).ok()?,
            step: f[1].parse().ok()?,
            operator: f[2].to_string(),
            name: f[3].to_string(),
            splits: f[4].parse().ok()?,
            colliding: f[5].parse().ok()?,
            adopted: f[6] == "1",
            cost: f[7].parse().ok()?,
            status_before: f[8].to_string(),
            status_after: f[9].to_string(),
            class: Class {
                objects: f[10].parse().ok()?,
                vocabulary: f[11].parse().ok()?,
                labelled: f[12].parse().ok()?,
                colliding: f[5].parse().ok()?,
            },
            conflict: f[13] == "1",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Record {
        Record {
            fingerprint: 0x00aa,
            step: 0,
            operator: "vsa-retriever".to_string(),
            name: "a&b".to_string(),
            splits: 2,
            colliding: 2,
            adopted: true,
            cost: 0.010,
            status_before: "INSUFFICIENT".to_string(),
            status_after: "IDENTIFIED".to_string(),
            class: Class { objects: 4, vocabulary: 1, labelled: 4, colliding: 2 },
            conflict: false,
        }
    }

    #[test]
    fn a_record_survives_a_round_trip() {
        let record = sample();
        assert_eq!(Record::parse(&record.to_line()), Some(record));
    }

    #[test]
    fn spaces_in_a_name_are_kept_and_tabs_are_not() {
        let mut record = sample();
        record.name = "b (if label 2 is wrong)\tand more".to_string();
        let parsed = Record::parse(&record.to_line()).expect("parse");
        assert!(parsed.name.contains("if label 2 is wrong"));
        assert!(!parsed.name.contains('\t'));
    }

    #[test]
    fn the_header_and_short_lines_are_not_records() {
        assert_eq!(Record::parse(VERSION), None);
        assert_eq!(Record::parse("# a comment"), None);
        assert_eq!(Record::parse("00aa\t0\top"), None);
    }
}
