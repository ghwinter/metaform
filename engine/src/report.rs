//! `report`: render the current project state as human-readable markdown (`.metaform/out/report.md`).
//!
//! Why it exists: JSON is for machines, but 'where exactly am I stuck right now' needs a paragraph for a human.
//! The report is also an **archivable artifact** — every judgement leaves behind the basis it was made on.

use std::path::Path;

use metaform::Status;

use crate::ledger;
use crate::plan;
use crate::state;

pub fn render(project: &Path) -> Result<String, String> {
    let dir = project.join(".metaform");
    let st = state::State::load(&dir.join("state.json"))?;
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
    // if a candidate pool exists, compute coverage from the pool; otherwise fall back to the current vocabulary
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
    md.push_str(&format!("Project: `{}`\n\n", project.display().to_string().replace('\\', "/")));
    md.push_str("## Status\n\n");
    md.push_str(&format!("- **{status}** → action `{action}` (θ={theta})\n"));
    md.push_str(&format!("- objects {}, predicates {}, votes {}\n", st.n, st.predicates.len(), st.votes.len()));
    md.push_str(&format!("- equivalence classes {}, open classes {}\n", e.classes().len(), free.len()));
    if pairs.is_empty() {
        md.push_str("- colliding pairs: none\n");
    } else {
        let shown: Vec<String> = pairs.iter().take(8).map(|(a, b)| format!("({a},{b})")).collect();
        md.push_str(&format!("- colliding pairs {}: {}\n", pairs.len(), shown.join(" ")));
    }

    md.push_str("\n## Coverage\n\n");
    if p.cover.is_empty() {
        md.push_str("- the current candidate pool cannot split any colliding pair (it needs new atoms, not new combinations)\n");
    } else {
        md.push_str(&format!(
            "- greedy choice {}: covers {}/{} pairs, {} remaining\n",
            p.cover.join(" → "),
            p.covered,
            p.pairs,
            p.remaining
        ));
    }

    md.push_str("\n## To verify\n\n");
    if unverified.is_empty() {
        md.push_str("- none: in each equivalence class, the directly asked members are enough to support the conclusion\n");
    } else {
        let shown: Vec<String> = unverified.iter().map(|x| x.to_string()).collect();
        md.push_str(&format!(
            "- these members were never asked directly; their labels are inherited from their class: {}\n",
            shown.join(", ")
        ));
        md.push_str("- probe one of each before stopping; if a probe conflicts with the class label, the status falls back to insufficient\n");
    }

    md.push_str("\n## Evidence ledger\n\n");
    md.push_str(&format!(
        "- entries {}: active {} / stale {} / superseded {} / retracted {}\n",
        folded.len(),
        active,
        stale,
        superseded,
        retracted
    ));
    let conf: Vec<String> = conflicts.keys().map(|c| c.to_string()).collect();
    md.push_str(&format!("- conflicts (truth undetermined): {}\n", if conf.is_empty() { "none".into() } else { conf.join(", ") }));
    let ok: Vec<String> = confirmed.iter().map(|c| c.to_string()).collect();
    md.push_str(&format!("- confirmed: {}\n", if ok.is_empty() { "none".into() } else { ok.join(", ") }));
    md.push_str(&format!(
        "- purity p={:.2} ({}), rework cost R={:.2} ({})\n",
        p.p_pure, p.p_source, p.rework, p.rework_source
    ));

    md.push_str("\n## Recent evidence\n\n");
    let recent: Vec<_> = folded.iter().rev().take(5).collect();
    if recent.is_empty() {
        md.push_str("- the ledger is empty\n");
    } else {
        for e in recent {
            let tail = if e.reason.is_empty() {
                String::new()
            } else {
                format!(" ({})", e.reason)
            };
            md.push_str(&format!(
                "- {} case {} by {} → {}{} \n",
                e.id,
                e.case,
                e.by,
                if e.answer { "yes" } else { "no" },
                tail
            ));
        }
    }

    md.push_str("\n## Next step\n\n");
    match action {
        "PROPOSE" => {
            md.push_str("- vocabulary insufficient: add predicates from the candidate pool (see \"Coverage\" above), then review again.\n");
        }
        "ASK" => {
            md.push_str(&format!("- open classes remain: suggested objects to label {:?}\n", free));
        }
        "VERIFY" => {
            md.push_str(&format!(
                "- probe these members with inherited labels before stopping: {:?}\n",
                unverified
            ));
        }
        _ => md.push_str("- uniquely determined: you can stop asking.\n"),
    }
    if !conflicts.is_empty() {
        md.push_str(&format!(
            "- resolve conflicts first: object(s) {} must be re-asked (see `metaform ledger conflicts`)\n",
            conf.join(", ")
        ));
    }
    md.push_str("\n---\n\nGenerated by `metaform report`; data comes from `.metaform/state.json` and `.metaform/ledger.tsv`.\n");
    Ok(md)
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
            predicates: vec![state::PredSpec {
                name: "p".into(),
                ids: vec![0, 1],
                ..Default::default()
            }],
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
        let md = render(&dir).unwrap();
        assert!(md.contains("INSUFFICIENT"));
        assert!(md.contains("(0,1)"));
        assert!(md.contains("## Next step"));
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
            reason: "probe 4 collides with 0".into(),
        };
        crate::ledger::append(&dir.join(".metaform/ledger.tsv"), &e);
        let md = render(&dir).unwrap();
        assert!(md.contains("## Recent evidence"), "{md}");
        assert!(md.contains("probe 4 collides with 0"), "{md}");
        assert!(md.contains("L1 case 4 by probe → no"), "{md}");
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
                ..Default::default()
            }],
            votes: vec![
                state::VoteSpec { id: 0, yes: 3, no: 0 },
                state::VoteSpec { id: 1, yes: 3, no: 0 },
            ],
        };
        st.save(&dir.join(".metaform/state.json")).unwrap();
        let md = render(&dir).unwrap();
        assert!(md.contains("VERIFY"), "{md}");
        assert!(md.contains("## To verify"), "{md}");
        assert!(md.contains("probe one of each"), "{md}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_says_stop_when_identified() {
        let dir = std::env::temp_dir().join("metaform-report-identified");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".metaform")).unwrap();
        let st = state::State {
            n: 2,
            predicates: vec![state::PredSpec {
                name: "p".into(),
                ids: vec![0],
                ..Default::default()
            }],
            votes: vec![
                state::VoteSpec { id: 0, yes: 3, no: 0 },
                state::VoteSpec { id: 1, yes: 0, no: 3 },
            ],
        };
        st.save(&dir.join(".metaform/state.json")).unwrap();
        let md = render(&dir).unwrap();
        assert!(md.contains("IDENTIFIED"));
        assert!(md.contains("can stop asking"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

