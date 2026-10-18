//! Scenario persistence: write the current experiment into `<project>/.metaform/state.json`.
//!
//! The goal is a **one-step call**: the agent sends just one command at a time (add / vote / need),
//! without writing a script file first and without keeping a session in memory.

use std::path::Path;

use metaform::{bit, Explorer, Predicate, Status, Vocabulary};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct PredSpec {
    pub name: String,
    pub ids: Vec<usize>,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct VoteSpec {
    pub id: usize,
    pub yes: u8,
    pub no: u8,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct State {
    pub n: usize,
    #[serde(default)]
    pub predicates: Vec<PredSpec>,
    #[serde(default)]
    pub votes: Vec<VoteSpec>,
}

impl State {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).expect("序列化失败");
        std::fs::write(path, text)
    }

    /// Rebuild the kernel state machine from the persisted state.
    pub fn explorer(&self) -> Explorer {
        let mut vocab = Vocabulary::with(Vec::new());
        for p in &self.predicates {
            let mut m: u64 = 0;
            for id in &p.ids {
                if *id < 64 {
                    m |= bit(*id);
                }
            }
            vocab.add(Predicate::new(p.name.clone(), m));
        }
        let mut e = Explorer::new(self.n, vocab);
        for v in &self.votes {
            e.answer_votes(v.id, v.yes, v.no);
        }
        e
    }
}

pub fn ids_json(v: &[usize]) -> String {
    let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", items.join(","))
}

pub fn pairs_json(p: &[(usize, usize)]) -> String {
    let items: Vec<String> = p.iter().map(|(a, b)| format!("[{a},{b}]")).collect();
    format!("[{}]", items.join(","))
}

#[allow(dead_code)] // currently used by tests; need_json inlines the same logic, to be merged later
pub fn colliding_pairs(e: &Explorer) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for c in e.classes() {
        let ins: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (e.pos & bit(x)) != 0)
            .collect();
        let outs: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (e.neg & bit(x)) != 0)
            .collect();
        for a in &ins {
            for b in &outs {
                out.push((*a, *b));
            }
        }
    }
    out
}

pub fn free_reps(e: &Explorer) -> Vec<usize> {
    let known = e.pos | e.neg;
    e.classes()
        .iter()
        .filter(|c| (**c & known) == 0 && **c != 0)
        .map(|c| c.trailing_zeros() as usize)
        .collect()
}

/// Exactly the same JSON as `need` in script mode.
pub fn need_json(e: &Explorer, theta: Option<u8>) -> String {
    let theta = theta.unwrap_or_else(|| e.recommended_theta());
    let (sp, sn) = e.strong(theta);
    let mut pairs = Vec::new();
    for c in e.classes() {
        let ins: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sp & bit(x)) != 0)
            .collect();
        let outs: Vec<usize> = (0..e.n)
            .filter(|&x| (c & bit(x)) != 0 && (sn & bit(x)) != 0)
            .collect();
        for a in &ins {
            for b in &outs {
                pairs.push((*a, *b));
            }
        }
    }
    let free = free_reps(e);
    let unverified = e.unprobed();
    let (st, action) = match e.status_with(theta) {
        Status::Insufficient => ("INSUFFICIENT", "PROPOSE"),
        Status::Undetermined => ("UNDETERMINED", "ASK"),
        Status::Identified if !unverified.is_empty() => ("IDENTIFIED", "VERIFY"),
        Status::Identified => ("IDENTIFIED", "STOP"),
    };
    format!(
        "{{\"status\":\"{st}\",\"action\":\"{action}\",\"theta\":{theta},\"epsilon_hat\":{:.2},\"theta_rec\":{},\"classes\":{},\"free_classes\":{},\"colliding_pairs\":{},\"pending\":{},\"unverified\":{},\"suggested_labels\":{}}}",
        e.implied_epsilon(),
        e.recommended_theta(),
        e.classes().len(),
        free.len(),
        pairs_json(&pairs),
        ids_json(&e.pending(theta)),
        ids_json(&unverified),
        ids_json(&free)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_save_load() {
        let dir = std::env::temp_dir().join("metaform-state-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("state.json");
        let st = State {
            n: 4,
            predicates: vec![PredSpec {
                name: "p".into(),
                ids: vec![0, 1],
            }],
            votes: vec![
                VoteSpec { id: 0, yes: 3, no: 0 },
                VoteSpec { id: 2, yes: 0, no: 3 },
            ],
        };
        st.save(&path).unwrap();
        let back = State::load(&path);
        assert_eq!(back.n, 4);
        assert_eq!(back.predicates[0].name, "p");
        assert_eq!(back.votes.len(), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn explorer_rebuilds_classes_and_status() {
        let st = State {
            n: 4,
            predicates: vec![PredSpec {
                name: "p".into(),
                ids: vec![0, 1],
            }],
            votes: vec![
                VoteSpec { id: 0, yes: 3, no: 0 },
                VoteSpec { id: 1, yes: 0, no: 3 },
            ],
        };
        let e = st.explorer();
        assert_eq!(e.classes().len(), 2);
        assert!(matches!(e.status_with(3), Status::Insufficient));
        assert_eq!(colliding_pairs(&e), vec![(0, 1)]);
    }

    #[test]
    fn need_asks_for_a_probe_when_labels_were_inherited() {
        let st = State {
            n: 3,
            predicates: vec![PredSpec {
                name: "p".into(),
                ids: vec![0],
            }],
            votes: vec![
                VoteSpec { id: 0, yes: 3, no: 0 },
                VoteSpec { id: 1, yes: 3, no: 0 },
            ],
        };
        let e = st.explorer();
        assert!(matches!(e.status_with(3), Status::Identified));
        let json = need_json(&e, Some(3));
        assert!(json.contains("\"action\":\"VERIFY\""), "{json}");
        assert!(json.contains("\"unverified\":[2]"), "{json}");
    }
}

