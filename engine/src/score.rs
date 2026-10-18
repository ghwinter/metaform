//! `score`：给候选谓词打分。
//!
//! 与 Python `proposer.py` 的评分同构，但输入是**外延**（对象编号），不是可执行代码：
//!   coverage   —— 劈开多少正反例对
//!   precision/recall —— 与隐藏真值的重合度（离线评估）
//!   predictions —— 候选自报"我预测为正"的对象命中多少
//!   ledger     —— 是否排除了台账里已确认的正例
//!
//! 两个角色分开判：
//!   definition —— 可以当作 S 的完整定义（P=R=1 且不违反台账）
//!   cover      —— 只作为集合覆盖的元素（sound 且至少劈开一对）

use std::collections::BTreeSet;


#[derive(Default, Clone, Debug)]
pub struct Truth {
    pub pos: BTreeSet<usize>,
    pub neg: BTreeSet<usize>,
}

#[derive(Default, Clone, Debug)]
pub struct Candidate {
    pub name: String,
    pub ext: BTreeSet<usize>,
    pub predictions: BTreeSet<usize>,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub name: String,
    pub covers: usize,
    pub total: usize,
    pub precision: f64,
    pub recall: f64,
    pub pred_hits: usize,
    pub pred_total: usize,
    pub ledger_violations: usize,
    pub definition_ok: bool,
    pub cover_ok: bool,
}

/// 解析 `pos 1 2 3` / `neg 4 5` 形式（`#` 后注释）。
pub fn parse_truth(text: &str) -> Truth {
    let mut t = Truth::default();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(kind) = it.next() else { continue };
        let target = match kind {
            "pos" => &mut t.pos,
            "neg" => &mut t.neg,
            _ => continue,
        };
        for tok in it {
            if let Ok(id) = tok.parse::<usize>() {
                target.insert(id);
            }
        }
    }
    t
}

/// 解析候选：`name id id ...`，行末可跟 `| pred_id pred_id ...` 表示预测。
pub fn parse_candidates(text: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (left, right) = match line.split_once('|') {
            Some((l, r)) => (l.trim(), Some(r.trim())),
            None => (line, None),
        };
        let mut it = left.split_whitespace();
        let Some(name) = it.next() else { continue };
        let mut ext = BTreeSet::new();
        for tok in it {
            if let Ok(id) = tok.parse::<usize>() {
                ext.insert(id);
            }
        }
        let mut predictions = BTreeSet::new();
        if let Some(r) = right {
            for tok in r.split_whitespace() {
                if let Ok(id) = tok.parse::<usize>() {
                    predictions.insert(id);
                }
            }
        }
        out.push(Candidate {
            name: name.to_string(),
            ext,
            predictions,
        });
    }
    out
}

pub fn score(
    cands: &[Candidate],
    truth: &Truth,
    pairs: &[(usize, usize)],
    confirmed: &BTreeSet<usize>,
) -> Vec<Row> {
    let mut rows = Vec::new();
    for c in cands {
        let covers = pairs
            .iter()
            .filter(|(a, b)| c.ext.contains(a) != c.ext.contains(b))
            .count();
        let inter = c.ext.intersection(&truth.pos).count();
        let precision = if c.ext.is_empty() {
            0.0
        } else {
            inter as f64 / c.ext.len() as f64
        };
        let recall = if truth.pos.is_empty() {
            0.0
        } else {
            inter as f64 / truth.pos.len() as f64
        };
        let pred_hits = c.predictions.intersection(&truth.pos).count();
        let ledger_violations = confirmed.iter().filter(|x| !c.ext.contains(x)).count();
        let definition_ok = precision >= 0.999 && recall >= 0.999 && ledger_violations == 0;
        let cover_ok = precision >= 0.999 && covers > 0;
        rows.push(Row {
            name: c.name.clone(),
            covers,
            total: pairs.len(),
            precision,
            recall,
            pred_hits,
            pred_total: c.predictions.len(),
            ledger_violations,
            definition_ok,
            cover_ok,
        });
    }
    rows
}

pub fn to_json(rows: &[Row]) -> String {
    let items: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "{{\"name\":\"{}\",\"covers\":{},\"total\":{},\"precision\":{:.2},\"recall\":{:.2},\"pred_hits\":{},\"pred_total\":{},\"ledger_violations\":{},\"definition_ok\":{},\"cover_ok\":{}}}",
                r.name,
                r.covers,
                r.total,
                r.precision,
                r.recall,
                r.pred_hits,
                r.pred_total,
                r.ledger_violations,
                r.definition_ok,
                r.cover_ok
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(name: &str, ids: &[usize], preds: &[usize]) -> Candidate {
        Candidate {
            name: name.into(),
            ext: ids.iter().copied().collect(),
            predictions: preds.iter().copied().collect(),
        }
    }

    #[test]
    fn perfect_candidate_is_definition() {
        let truth = Truth {
            pos: [3usize, 6].into_iter().collect(),
            neg: [2usize].into_iter().collect(),
        };
        let rows = score(
            &[cand("exact", &[3, 6], &[3])],
            &truth,
            &[(3, 2)],
            &[3usize].into_iter().collect(),
        );
        assert!(rows[0].definition_ok);
        assert_eq!(rows[0].precision, 1.0);
        assert_eq!(rows[0].recall, 1.0);
        assert_eq!(rows[0].pred_hits, 1);
    }

    #[test]
    fn narrow_candidate_is_cover_but_not_definition() {
        let truth = Truth {
            pos: [3usize, 6].into_iter().collect(),
            neg: [2usize].into_iter().collect(),
        };
        let rows = score(
            &[cand("narrow", &[3], &[3])],
            &truth,
            &[(3, 2)],
            &BTreeSet::new(),
        );
        assert!(!rows[0].definition_ok);
        assert!(rows[0].cover_ok);
        assert_eq!(rows[0].recall, 0.5);
    }

    #[test]
    fn ledger_violation_blocks_definition() {
        let truth = Truth {
            pos: [3usize, 6].into_iter().collect(),
            neg: [2usize].into_iter().collect(),
        };
        let rows = score(
            &[cand("forgetful", &[3, 6], &[])],
            &truth,
            &[(3, 2)],
            &[3usize, 7].into_iter().collect(), // 7 是已确认正例，但候选排除了它
        );
        assert_eq!(rows[0].ledger_violations, 1);
        assert!(!rows[0].definition_ok);
    }
}

