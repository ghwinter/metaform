//! `report`：把当前项目的状态写成一份人读的 markdown（`.metaform/out/report.md`）。
//!
//! 为什么需要它：JSON 给机器看，但"我现在到底卡在哪"这件事，人需要一段话。
//! 报告同时是**可归档的产物**——每次判断都留下当时的依据。

use std::path::Path;

use metaform::Status;

use crate::ledger;
use crate::plan;
use crate::state;

pub fn render(project: &Path) -> String {
    let dir = project.join(".metaform");
    let st = state::State::load(&dir.join("state.json"));
    let rows = ledger::read_all(&dir.join("ledger.tsv"));
    let folded = ledger::fold(&rows);
    let e = st.explorer();
    let theta = e.recommended_theta();
    let pairs = plan::colliding(&e, theta);
    let unverified = e.unprobed();
    let (status, action) = match e.status_with(theta) {
        Status::Insufficient => ("INSUFFICIENT", "PROPOSE"),
        Status::Undetermined => ("UNDETERMINED", "ASK"),
        Status::Identified if !unverified.is_empty() => ("IDENTIFIED", "VERIFY"),
        Status::Identified => ("IDENTIFIED", "STOP"),
    };
    let free = state::free_reps(&e);
    let conflicts = ledger::conflicts(&rows, 0.5, 0.7);
    let confirmed = ledger::confirmed_cases(&rows, 0.5, 0.7);
    // 若存在候选池，报告按池计算覆盖；否则退回当前词汇
    let pool_path = dir.join("out").join("pool.txt");
    let pool = std::fs::read_to_string(&pool_path)
        .ok()
        .map(|t| plan::parse_pool(&t));
    let p = plan::build(&e, &rows, theta, pool.as_deref());

    let active = folded.iter().filter(|x| x.status == "active").count();
    let retracted = folded.iter().filter(|x| x.status == "retracted").count();
    let superseded = folded.iter().filter(|x| x.status == "superseded").count();
    let stale = folded.iter().filter(|x| x.status == "stale").count();

    let mut md = String::new();
    md.push_str("# metaform report\n\n");
    md.push_str(&format!("项目：`{}`\n\n", project.display().to_string().replace('\\', "/")));
    md.push_str("## 状态\n\n");
    md.push_str(&format!("- **{status}** → 动作 `{action}`（θ={theta}）\n"));
    md.push_str(&format!("- 对象 {} 个，谓词 {} 个，票 {} 条\n", st.n, st.predicates.len(), st.votes.len()));
    md.push_str(&format!("- 等价类 {} 个，未决类 {}\n", e.classes().len(), free.len()));
    if pairs.is_empty() {
        md.push_str("- 撞车对：无\n");
    } else {
        let shown: Vec<String> = pairs.iter().take(8).map(|(a, b)| format!("({a},{b})")).collect();
        md.push_str(&format!("- 撞车对 {} 个：{}\n", pairs.len(), shown.join(" ")));
    }

    md.push_str("\n## 覆盖\n\n");
    if p.cover.is_empty() {
        md.push_str("- 当前候选池无法劈开任何撞车对（需要新原子，而不是新组合）\n");
    } else {
        md.push_str(&format!(
            "- 贪心选择 {}：覆盖 {}/{} 对，剩余 {}\n",
            p.cover.join(" → "),
            p.covered,
            p.pairs,
            p.remaining
        ));
    }

    md.push_str("\n## 待验证\n\n");
    if unverified.is_empty() {
        md.push_str("- 无：每个等价类里被直接问过的成员足以支撑结论\n");
    } else {
        let shown: Vec<String> = unverified.iter().map(|x| x.to_string()).collect();
        md.push_str(&format!(
            "- 这些成员没被直接问过，标签是从同类继承的：{}\n",
            shown.join(", ")
        ));
        md.push_str("- 停机之前各探一个；探测若与类标签冲突，状态会退回不足\n");
    }

    md.push_str("\n## 证据台账\n\n");
    md.push_str(&format!(
        "- 条目 {}：active {} / stale {} / superseded {} / retracted {}\n",
        folded.len(),
        active,
        stale,
        superseded,
        retracted
    ));
    let conf: Vec<String> = conflicts.keys().map(|c| c.to_string()).collect();
    md.push_str(&format!("- 冲突（真值未定）：{}\n", if conf.is_empty() { "无".into() } else { conf.join(", ") }));
    let ok: Vec<String> = confirmed.iter().map(|c| c.to_string()).collect();
    md.push_str(&format!("- 已确认：{}\n", if ok.is_empty() { "无".into() } else { ok.join(", ") }));
    md.push_str(&format!(
        "- 纯度 p={:.2}（{}），返工代价 R={:.2}（{}）\n",
        p.p_pure, p.p_source, p.rework, p.rework_source
    ));

    md.push_str("\n## 最近证据\n\n");
    let recent: Vec<_> = folded.iter().rev().take(5).collect();
    if recent.is_empty() {
        md.push_str("- 台账为空\n");
    } else {
        for e in recent {
            let tail = if e.reason.is_empty() {
                String::new()
            } else {
                format!("（{}）", e.reason)
            };
            md.push_str(&format!(
                "- {} case {} by {} → {}{} \n",
                e.id,
                e.case,
                e.by,
                if e.answer { "是" } else { "否" },
                tail
            ));
        }
    }

    md.push_str("\n## 下一步\n\n");
    match action {
        "PROPOSE" => {
            md.push_str("- 词汇不够：从候选池里补谓词（见上「覆盖」），补完再审。\n");
        }
        "ASK" => {
            md.push_str(&format!("- 还有未决类：建议标注对象 {:?}\n", free));
        }
        "VERIFY" => {
            md.push_str(&format!(
                "- 先探测这些继承标签的成员再停：{:?}\n",
                unverified
            ));
        }
        _ => md.push_str("- 已唯一确定：可以停止提问。\n"),
    }
    if !conflicts.is_empty() {
        md.push_str(&format!(
            "- 先解决冲突：{} 号对象需要重问（见 `metaform ledger conflicts`）\n",
            conf.join(", ")
        ));
    }
    md.push_str("\n---\n\n由 `metaform report` 生成；数据来自 `.metaform/state.json` 与 `.metaform/ledger.tsv`。\n");
    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("metaform-report-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".metaform")).unwrap();
        let st = state::State {
            n: 4,
            predicates: vec![state::PredSpec { name: "p".into(), ids: vec![0, 1] }],
            votes: vec![
                state::VoteSpec { id: 0, yes: 3, no: 0 },
                state::VoteSpec { id: 1, yes: 0, no: 3 },
            ],
        };
        st.save(&dir.join(".metaform/state.json")).unwrap();
        dir
    }

    #[test]
    fn report_marks_insufficient_and_names_collision() {
        let dir = setup("insufficient");
        let md = render(&dir);
        assert!(md.contains("INSUFFICIENT"));
        assert!(md.contains("(0,1)"));
        assert!(md.contains("## 下一步"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_lists_recent_ledger_evidence() {
        let dir = setup("ledger-evidence");
        let e = crate::ledger::Entry {
            id: "L1".into(),
            case: 4,
            status: "active".into(),
            by: "probe".into(),
            round: 0,
            reliability: 1.0,
            answer: false,
            reason: "探测 4：与 0 撞车".into(),
        };
        crate::ledger::append(&dir.join(".metaform/ledger.tsv"), &e);
        let md = render(&dir);
        assert!(md.contains("## 最近证据"), "{md}");
        assert!(md.contains("探测 4：与 0 撞车"), "{md}");
        assert!(md.contains("L1 case 4 by probe → 否"), "{md}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_asks_for_probes_before_stopping() {
        let dir = std::env::temp_dir().join("metaform-report-verify");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".metaform")).unwrap();
        let st = state::State {
            n: 3,
            predicates: vec![state::PredSpec {
                name: "p".into(),
                ids: vec![0],
            }],
            votes: vec![
                state::VoteSpec { id: 0, yes: 3, no: 0 },
                state::VoteSpec { id: 1, yes: 3, no: 0 },
            ],
        };
        st.save(&dir.join(".metaform/state.json")).unwrap();
        let md = render(&dir);
        assert!(md.contains("VERIFY"), "{md}");
        assert!(md.contains("## 待验证"), "{md}");
        assert!(md.contains("先探测"), "{md}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_says_stop_when_identified() {
        let dir = std::env::temp_dir().join("metaform-report-identified");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".metaform")).unwrap();
        let st = state::State {
            n: 2,
            predicates: vec![state::PredSpec { name: "p".into(), ids: vec![0] }],
            votes: vec![
                state::VoteSpec { id: 0, yes: 3, no: 0 },
                state::VoteSpec { id: 1, yes: 0, no: 3 },
            ],
        };
        st.save(&dir.join(".metaform/state.json")).unwrap();
        let md = render(&dir);
        assert!(md.contains("IDENTIFIED"));
        assert!(md.contains("可以停止提问"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

