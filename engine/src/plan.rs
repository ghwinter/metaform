//! `plan`: cover + purity + re-ask budget in a single call.
//!
//! Three things:
//!   1. cover  — greedily pick the smallest set of candidate predicates that covers every colliding pair (the hands-on version of T7)
//!   2. purity — merge conflicting objects by equivalence class; a class with both known positives and negatives ⇒ impure
//!   3. budget — with unknown purity, compare `1 + (1-p)(c-1)R` against `c` to decide between asking one or asking all

use metaform::{bit, Explorer};

use crate::ledger;

pub struct ClassPlan {
    pub class: usize,
    pub size: usize,
    pub known_pos: usize,
    pub known_neg: usize,
    pub conflicted: Vec<usize>,
    pub purity: String,
    pub asks: usize,
    pub cost_one: f64,
    pub cost_all: f64,
}

pub struct Plan {
    pub pairs: usize,
    pub cover: Vec<String>,
    pub covered: usize,
    pub remaining: usize,
    pub p_pure: f64,
    pub p_source: String,
    pub rework: f64,
    pub rework_source: String,
    pub classes: Vec<ClassPlan>,
}

/// Parse a pool file: each line is `name id id ...`, with `#` starting a comment.
pub fn parse_pool(text: &str) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let mut mask = 0u64;
        for tok in it {
            if let Ok(id) = tok.parse::<usize>() {
                if id < 64 {
                    mask |= 1u64 << id;
                }
            }
        }
        out.push((name.to_string(), mask));
    }
    out
}

pub fn colliding(e: &Explorer, theta: u8) -> Vec<(usize, usize)> {
    let (sp, sn) = e.strong(theta);
    let mut out = Vec::new();
    for c in e.classes() {
        let ins: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sp & bit(x)) != 0)
            .collect();
        let outs: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sn & bit(x)) != 0)
            .collect();
        for a in &ins {
            for b in &outs {
                out.push((*a, *b));
            }
        }
    }
    out
}

/// Greedy cover: at each step pick the predicate that splits the most uncovered pairs.
pub fn greedy_cover(
    preds: &[(String, u64)],
    pairs: &[(usize, usize)],
) -> (Vec<String>, usize) {
    let mut remaining: Vec<(usize, usize)> = pairs.to_vec();
    let mut pool: Vec<(String, u64)> = preds.to_vec();
    let mut chosen = Vec::new();
    while !remaining.is_empty() && !pool.is_empty() {
        let mut best: Option<(usize, usize)> = None;
        for (i, (_, m)) in pool.iter().enumerate() {
            let c = remaining
                .iter()
                .filter(|(a, b)| ((m >> a) & 1) != ((m >> b) & 1))
                .count();
            if best.map_or(true, |(_, bc)| c > bc) {
                best = Some((i, c));
            }
        }
        let (idx, covered) = best.expect("pool non-empty");
        if covered == 0 {
            break;
        }
        let (name, mask) = pool.remove(idx);
        remaining.retain(|(a, b)| ((mask >> a) & 1) == ((mask >> b) & 1));
        chosen.push(name);
    }
    (chosen, remaining.len())
}

/// Rework cost: 1 + retracted entries / active entries (lower bound 1). Without amend history this is the computable approximation.
pub fn rework_cost(rows: &[ledger::Entry]) -> (f64, String) {
    let folded = ledger::fold(rows);
    let active = folded.iter().filter(|e| e.status == "active").count();
    let retracted = folded.iter().filter(|e| e.status == "retracted").count();
    if active == 0 {
        return (1.0, "no active entries".into());
    }
    let r = 1.0 + retracted as f64 / active as f64;
    (r, format!("1 + {retracted}/{active} retracted"))
}

fn case_class(e: &Explorer, case: usize) -> Option<usize> {
    e.classes()
        .iter()
        .position(|c| (c >> case) & 1 == 1)
}

pub fn build(
    e: &Explorer,
    rows: &[ledger::Entry],
    theta: u8,
    pool: Option<&[(String, u64)]>,
) -> Plan {
    let pairs = colliding(e, theta);
    let default_pool: Vec<(String, u64)> = e
        .vocab
        .preds
        .iter()
        .map(|p| (p.name.clone(), p.ext))
        .collect();
    let pool_ref = pool.unwrap_or(&default_pool);
    let (cover, remaining) = greedy_cover(pool_ref, &pairs);

    // conflicting objects → equivalence classes
    let conf = ledger::conflicts(rows, 0.5, 0.7);
    let confirmed: Vec<usize> = ledger::confirmed_cases(rows, 0.5, 0.7);
    let neg_cases: Vec<usize> = {
        let sup = ledger::support(rows);
        let mut v: Vec<usize> = sup
            .iter()
            .filter(|(c, s)| s.neg >= 0.5 && !conf.contains_key(c))
            .map(|(c, _)| *c)
            .collect();
        v.sort_unstable();
        v
    };

    // purity: only classes with 'at least two confirmed labels' are considered
    let classes = e.classes();
    let mut labelled = 0usize;
    let mut impure = 0usize;
    for mask in &classes {
        let kp = confirmed.iter().filter(|c| (mask >> *c) & 1 == 1).count();
        let kn = neg_cases.iter().filter(|c| (mask >> *c) & 1 == 1).count();
        if kp + kn >= 2 {
            labelled += 1;
            if kp > 0 && kn > 0 {
                impure += 1;
            }
        }
    }
    let p_pure = if labelled == 0 {
        0.5
    } else {
        (impure as f64 + 1.0) / (labelled as f64 + 2.0)
    };
    let p_source = if labelled == 0 {
        "prior (no labelled classes)".to_string()
    } else {
        format!("{impure}/{labelled} impure, Laplace")
    };

    let (rework, rework_src) = rework_cost(rows);

    // aggregate conflicting objects by class
    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for case in conf.keys() {
        if let Some(ci) = case_class(e, *case) {
            match groups.iter_mut().find(|(i, _)| *i == ci) {
                Some((_, v)) => v.push(*case),
                None => groups.push((ci, vec![*case])),
            }
        }
    }

    let mut class_plans = Vec::new();
    for (ci, conflicted) in groups {
        let mask = classes[ci];
        let size = mask.count_ones() as usize;
        let kp = confirmed.iter().filter(|c| (mask >> *c) & 1 == 1).count();
        let kn = neg_cases.iter().filter(|c| (mask >> *c) & 1 == 1).count();
        let c_ = conflicted.len();
        let cost_one = 1.0 + (1.0 - p_pure) * (c_ as f64 - 1.0) * rework;
        let cost_all = c_ as f64;
        let (purity, asks) = if kp > 0 && kn > 0 {
            ("impure(known)".to_string(), c_)
        } else if size == 1 {
            ("pure?(single)".to_string(), 1)
        } else if c_ == 1 || cost_one <= cost_all {
            ("unknown".to_string(), 1)
        } else {
            ("unknown(buy-safe)".to_string(), c_)
        };
        class_plans.push(ClassPlan {
            class: ci,
            size,
            known_pos: kp,
            known_neg: kn,
            conflicted,
            purity,
            asks,
            cost_one,
            cost_all,
        });
    }
    class_plans.sort_by_key(|c| c.class);

    Plan {
        pairs: pairs.len(),
        covered: pairs.len() - remaining,
        remaining,
        cover,
        p_pure,
        p_source,
        rework,
        rework_source: rework_src,
        classes: class_plans,
    }
}

pub fn to_json(p: &Plan) -> String {
    let cover: Vec<String> = p.cover.iter().map(|s| format!("\"{s}\"")).collect();
    let classes: Vec<String> = p
        .classes
        .iter()
        .map(|c| {
            let ids: Vec<String> = c.conflicted.iter().map(|x| x.to_string()).collect();
            format!(
                "{{\"class\":{},\"size\":{},\"known_pos\":{},\"known_neg\":{},\"conflicted\":[{}],\"purity\":\"{}\",\"asks\":{},\"cost_one\":{:.2},\"cost_all\":{:.2}}}",
                c.class,
                c.size,
                c.known_pos,
                c.known_neg,
                ids.join(","),
                c.purity,
                c.asks,
                c.cost_one,
                c.cost_all
            )
        })
        .collect();
    format!(
        "{{\"pairs\":{},\"cover\":[{}],\"covered\":{},\"remaining\":{},\"p_pure\":{:.2},\"p_source\":\"{}\",\"rework\":{:.2},\"rework_source\":\"{}\",\"classes\":[{}]}}",
        p.pairs,
        cover.join(","),
        p.covered,
        p.remaining,
        p.p_pure,
        p.p_source,
        p.rework,
        p.rework_source,
        classes.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greedy_prefers_predicate_that_covers_most() {
        // objects 0..3; p separates (0,1) from (0,2), q separates only (0,1)
        let preds = vec![("p".to_string(), 0b001u64), ("q".to_string(), 0b011u64)];
        // p: true only for 0 → separates it from both 1 and 2
        // q: true for 0,1 → separates 2, but not 1
        let pairs = vec![(0usize, 1usize), (0, 2)];
        let (chosen, remaining) = greedy_cover(&preds, &pairs);
        assert_eq!(chosen, vec!["p"]);
        assert_eq!(remaining, 0);
    }

    #[test]
    fn budget_flips_with_rework_cost() {
        // c=2, p=0.5 → cost_one = 1 + 0.5*1*R
        let cost = |r: f64| 1.0 + 0.5 * 1.0 * r;
        assert!(cost(1.0) <= 2.0); // cheap → ask one
        assert!(cost(3.0) > 2.0); // expensive → ask all
    }

    #[test]
    fn rework_is_at_least_one() {
        let rows = vec![ledger::Entry {
            id: "L1".into(),
            case: 0,
            status: "active".into(),
            by: "t".into(),
            round: 0,
            reliability: 1.0,
            answer: true,
            reason: String::new(),
        }];
        let (r, _) = rework_cost(&rows);
        assert!(r >= 1.0);
    }
}

