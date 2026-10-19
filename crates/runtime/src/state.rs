//! A line-format state file, so a scenario can hand a state to a component without JSON.
//!
//! ```text
//! objects 6
//! vocab a 0 1 3
//! attr b 1 2 5
//! pos 0
//! neg 1
//! systems <path>        (optional, for the analogy component)
//! ```text
//!
//! The format is deliberately small: a component reads a state, and writes proposals in the
//! engine's `metaform.propose/1` response format, which the engine then validates.

use metaform::{bit, Predicate, Vocabulary};
use std::collections::BTreeSet;

/// What a component needs to see of a state.
#[derive(Clone, Debug)]
pub struct State {
    pub objects: usize,
    pub vocabulary: Vocabulary,
    pub attributes: Vec<(String, u64)>,
    pub pos: u64,
    pub neg: u64,
    /// Optional path to a relational-system file for the analogy component.
    pub systems: Option<String>,
    pub request_id: String,
}

impl State {
    pub fn read(path: &str) -> Result<State, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
        let mut objects = 0usize;
        let mut vocabulary: Vec<Predicate> = Vec::new();
        let mut attributes: Vec<(String, u64)> = Vec::new();
        let mut pos = 0u64;
        let mut neg = 0u64;
        let mut systems = None;
        let mut request_id = "state".to_string();

        for (lineno, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() || fields[0].starts_with('#') {
                continue;
            }
            let mask = |rest: &[&str]| -> u64 {
                rest.iter().filter_map(|f| f.parse::<usize>().ok()).fold(0u64, |m, i| m | bit(i))
            };
            match fields[0] {
                "objects" => {
                    objects = fields
                        .get(1)
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| format!("line {}: objects needs a count", lineno + 1))?;
                }
                "vocab" => {
                    if fields.len() < 2 {
                        return Err(format!("line {}: vocab needs a name", lineno + 1));
                    }
                    vocabulary.push(Predicate::new(fields[1], mask(&fields[2..])));
                }
                "attr" => {
                    if fields.len() < 2 {
                        return Err(format!("line {}: attr needs a name", lineno + 1));
                    }
                    attributes.push((fields[1].to_string(), mask(&fields[2..])));
                }
                "pos" => pos |= mask(&fields[1..]),
                "neg" => neg |= mask(&fields[1..]),
                "systems" => systems = fields.get(1).map(|s| s.to_string()),
                "request_id" => {
                    if let Some(v) = fields.get(1) {
                        request_id = v.to_string();
                    }
                }
                other => return Err(format!("line {}: unknown directive {}", lineno + 1, other)),
            }
        }

        if objects == 0 {
            return Err("the state has no objects".to_string());
        }
        Ok(State {
            objects,
            vocabulary: Vocabulary::with(vocabulary),
            attributes,
            pos,
            neg,
            systems,
            request_id,
        })
    }

    pub fn positives(&self) -> Vec<usize> {
        (0..self.objects).filter(|i| (self.pos >> i) & 1 == 1).collect()
    }

    pub fn negatives(&self) -> Vec<usize> {
        (0..self.objects).filter(|i| (self.neg >> i) & 1 == 1).collect()
    }

    /// The engine-side context this component is allowed to see.
    pub fn context(&self) -> crate::Context {
        crate::Context {
            objects: self.objects,
            vocabulary: self.vocabulary.clone(),
            attributes: self.attributes.clone(),
            pos: self.pos,
            neg: self.neg,
            label: self.request_id.clone(),
        }
    }
}

/// A relational system file for the analogy component.
///
/// ```texttext
/// base entities 2
/// base stmt greater e0 e1
/// base stmt flow e0 e1
/// base stmt cause s0 s1
/// target entities 2
/// target stmt greater e0 e1
/// ...
/// reading liquid 2 5
/// ```text
pub struct Systems {
    pub base: metaform_sme::System,
    pub target: metaform_sme::System,
    pub readings: Vec<(String, u64)>,
}

impl Systems {
    pub fn read(path: &str) -> Result<Systems, String> {
        use metaform_sme::Term;
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
        let mut base = metaform_sme::System::new(0);
        let mut target = metaform_sme::System::new(0);
        let mut readings: Vec<(String, u64)> = Vec::new();

        for (lineno, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.is_empty() || fields[0].starts_with('#') {
                continue;
            }
            let side = fields[0];
            match (side, fields.get(1).copied()) {
                ("base", Some("entities")) | ("target", Some("entities")) => {
                    let n: usize = fields
                        .get(2)
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(|| format!("line {}: entities needs a count", lineno + 1))?;
                    if side == "base" {
                        base = metaform_sme::System::new(n);
                    } else {
                        target = metaform_sme::System::new(n);
                    }
                }
                ("base", Some("stmt")) | ("target", Some("stmt")) => {
                    if fields.len() < 4 {
                        return Err(format!("line {}: stmt needs a relation and arguments", lineno + 1));
                    }
                    let mut args = Vec::new();
                    for a in &fields[3..] {
                        if let Some(rest) = a.strip_prefix('e') {
                            args.push(Term::Entity(rest.parse().map_err(|_| {
                                format!("line {}: bad entity {}", lineno + 1, a)
                            })?));
                        } else if let Some(rest) = a.strip_prefix('s') {
                            args.push(Term::Statement(rest.parse().map_err(|_| {
                                format!("line {}: bad statement {}", lineno + 1, a)
                            })?));
                        } else {
                            return Err(format!("line {}: argument {} is neither eN nor sN", lineno + 1, a));
                        }
                    }
                    if side == "base" {
                        base.push(fields[2], args);
                    } else {
                        target.push(fields[2], args);
                    }
                }
                ("reading", Some(name)) => {
                    let mask = fields[2..]
                        .iter()
                        .filter_map(|f| f.parse::<usize>().ok())
                        .fold(0u64, |m, i| m | bit(i));
                    readings.push((name.to_string(), mask));
                }
                // State directives may live in the same file; they are not this reader's business.
                _ => {}
            }
        }
        let _ = BTreeSet::<u8>::new();
        Ok(Systems { base, target, readings })
    }
}
