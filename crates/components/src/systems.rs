//! A relational system file for the analogy component.
//!
//! ```text
//! base entities 2
//! base stmt greater e0 e1
//! base stmt flow e0 e1
//! base stmt cause s0 s1
//! target entities 2
//! target stmt greater e0 e1
//! ...
//! reading liquid 2 5
//! ```

use metaform::bit;

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
        Ok(Systems { base, target, readings })
    }
}
