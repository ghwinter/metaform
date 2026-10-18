//! 证据台账：只追加的 TSV 日志，按 id 折叠出最新状态。
//!
//! 为什么只追加：撤销/修改都写成新行（同 id，后者胜），文件永远不被改写，
//! 既不会写坏，也保留了完整历史。
//!
//! 列：id  case  status  by  round  reliability  answer  reason

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub const HEADER: &str =
    "# metaform ledger v1\nid\tcase\tstatus\tby\tround\treliability\tanswer\treason\n";

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub case: usize,
    pub status: String,
    pub by: String,
    pub round: u32,
    pub reliability: f64,
    pub answer: bool,
    pub reason: String,
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

pub fn read_all(path: &Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 8 {
                return None;
            }
            Some(Entry {
                id: f[0].to_string(),
                case: f[1].parse().ok()?,
                status: f[2].to_string(),
                by: f[3].to_string(),
                round: f[4].parse().ok()?,
                reliability: f[5].parse().ok()?,
                answer: f[6] == "true",
                reason: f[7].to_string(),
            })
        })
        .collect()
}

/// 按 id 折叠：后出现的行覆盖先前的，顺序保留首次出现的位置。
pub fn fold(entries: &[Entry]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for e in entries {
        match out.iter_mut().find(|x| x.id == e.id) {
            Some(slot) => *slot = e.clone(),
            None => out.push(e.clone()),
        }
    }
    out
}

pub fn next_id(entries: &[Entry]) -> String {
    let max = entries
        .iter()
        .filter_map(|e| e.id.strip_prefix('L').and_then(|n| n.parse::<usize>().ok()))
        .max()
        .unwrap_or(0);
    format!("L{}", max + 1)
}

/// 同一对象的正、负支持强度（独立证据按 noisy-OR 合并：1 - Π(1 - r)）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Support {
    pub pos: f64,
    pub neg: f64,
}

impl Support {
    fn add(&mut self, positive: bool, r: f64) {
        let cur = if positive { self.pos } else { self.neg };
        let next = 1.0 - (1.0 - cur) * (1.0 - r);
        if positive {
            self.pos = next;
        } else {
            self.neg = next;
        }
    }
}

/// 只统计 active 条目，按 case 折叠方向。
pub fn support(entries: &[Entry]) -> std::collections::BTreeMap<usize, Support> {
    let mut out: std::collections::BTreeMap<usize, Support> = Default::default();
    for e in fold(entries) {
        if e.status != "active" {
            continue;
        }
        out.entry(e.case).or_default().add(e.answer, e.reliability);
    }
    out
}

/// 冲突：两侧都有分量（都 ≥ threshold），且弱者不弱于强者 ratio 倍。
/// 压倒性的证据不会被弱反对票取消。
pub fn conflicts(
    entries: &[Entry],
    threshold: f64,
    ratio: f64,
) -> std::collections::BTreeMap<usize, Support> {
    let mut out = std::collections::BTreeMap::new();
    for (case, s) in support(entries) {
        let (lo, hi) = if s.pos < s.neg {
            (s.pos, s.neg)
        } else {
            (s.neg, s.pos)
        };
        if lo >= threshold && hi > 0.0 && lo >= ratio * hi {
            out.insert(case, s);
        }
    }
    out
}

/// 已确认且无冲突的对象：正支持 ≥ threshold，且不在 conflicts 里。
pub fn confirmed_cases(
    entries: &[Entry],
    threshold: f64,
    ratio: f64,
) -> Vec<usize> {
    let conf = conflicts(entries, threshold, ratio);
    let mut v: Vec<usize> = support(entries)
        .into_iter()
        .filter(|(case, s)| s.pos >= threshold && !conf.contains_key(case))
        .map(|(case, _)| case)
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

pub fn append(path: &Path, e: &Entry) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let new_file = !path.exists();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("打不开台账");
    if new_file {
        let _ = f.write_all(HEADER.as_bytes());
    }
    let line = format!(
        "{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{}\n",
        clean(&e.id),
        e.case,
        clean(&e.status),
        clean(&e.by),
        e.round,
        e.reliability,
        e.answer,
        clean(&e.reason)
    );
    let _ = f.write_all(line.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, status: &str, case: usize, answer: bool) -> Entry {
        Entry {
            id: id.into(),
            case,
            status: status.into(),
            by: "test".into(),
            round: 0,
            reliability: 1.0,
            answer,
            reason: String::new(),
        }
    }

    #[test]
    fn fold_keeps_latest_status() {
        let rows = vec![
            e("L1", "active", 3, true),
            e("L2", "active", 4, true),
            e("L1", "retracted", 3, true),
        ];
        let f = fold(&rows);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].id, "L1");
        assert_eq!(f[0].status, "retracted");
    }

    #[test]
    fn active_cases_skip_retracted_and_negative() {
        let rows = vec![
            e("L1", "active", 3, true),
            e("L2", "retracted", 4, true),
            e("L3", "active", 5, false),
        ];
        let mut cases: Vec<usize> = fold(&rows)
            .iter()
            .filter(|x| x.status == "active" && x.answer)
            .map(|x| x.case)
            .collect();
        cases.sort_unstable();
        assert_eq!(cases, vec![3]);
    }
}

#[cfg(test)]
mod support_tests {
    use super::*;

    fn e(id: &str, status: &str, case: usize, answer: bool, r: f64) -> Entry {
        Entry {
            id: id.into(),
            case,
            status: status.into(),
            by: "t".into(),
            round: 0,
            reliability: r,
            answer,
            reason: String::new(),
        }
    }

    #[test]
    fn support_combines_noisy_or() {
        let rows = vec![
            e("L1", "active", 3, true, 0.5),
            e("L2", "active", 3, true, 0.5),
        ];
        let s = support(&rows);
        assert!((s[&3].pos - 0.75).abs() < 1e-9);
        assert_eq!(s[&3].neg, 0.0);
    }

    #[test]
    fn conflict_needs_comparable_sides() {
        let close = vec![
            e("L1", "active", 4, true, 0.9),
            e("L2", "active", 4, false, 0.9),
        ];
        assert!(conflicts(&close, 0.5, 0.7).contains_key(&4));

        let lopsided = vec![
            e("L1", "active", 5, true, 1.0),
            e("L2", "active", 5, false, 0.5),
        ];
        assert!(!conflicts(&lopsided, 0.5, 0.7).contains_key(&5));
    }

    #[test]
    fn confirmed_cases_exclude_conflicts() {
        let rows = vec![
            e("L1", "active", 3, true, 1.0),
            e("L2", "active", 4, true, 0.9),
            e("L3", "active", 4, false, 0.9),
            e("L4", "retracted", 5, true, 1.0),
        ];
        assert_eq!(confirmed_cases(&rows, 0.5, 0.7), vec![3]);
    }
}
