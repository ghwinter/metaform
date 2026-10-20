//! Scenario persistence: write the current experiment into `<project>/.metaform/state.json`.
//!
//! The goal is a **one-step call**: the agent sends just one command at a time (add / vote / need),
//! without writing a script file first and without keeping a session in memory.
//!
//! A predicate may carry a `definition` (`all_of` / `any_of` over existing predicate names). The file
//! stores the definition next to the extension; loading recomputes the extension against the current
//! object count, so growing the project does not leave a stale mask behind.

use std::collections::HashMap;
use std::path::Path;

use crate::{bit, DefOp, Definition, Explorer, Predicate, Status, Vocabulary};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct PredSpec {
    pub name: String,
    /// The extension at save time. For a defined predicate this is a cache: it is recomputed on load.
    /// Missing ids are allowed (a definition-only entry loads fine); the extension is then derived.
    #[serde(default)]
    pub ids: Vec<usize>,
    /// How the extension was defined. Missing (`None`) means the predicate is extensional, which keeps
    /// old state files readable. `define` is accepted as an alias when reading.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "define")]
    pub definition: Option<Definition>,
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
    /// Read the state file and resolve every stored definition.
    ///
    /// A missing (or empty) file loads as the default empty scenario, exactly as before. Any other
    /// read/parse failure and any unresolvable definition is an error: a definition is never dropped
    /// silently, because dropping it would leave a predicate whose extension no longer follows from
    /// the object set.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("cannot read `{}`: {e}", path.display())),
        };
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let mut st: Self = serde_json::from_str(&text)
            .map_err(|e| format!("cannot parse `{}`: {e}", path.display()))?;
        st.resolve()?;
        Ok(st)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).expect("serialization failed");
        std::fs::write(path, text)
    }

    /// Recompute the extension of every defined predicate from its operands and the current object
    /// count. Definitions are evaluated in dependency order; a missing operand and a dependency cycle
    /// are both errors (naming the operand / the cycle) rather than silent drops or infinite recursion.
    ///
    /// Extensional predicates are left untouched, so loading an old file is a no-op.
    pub fn resolve(&mut self) -> Result<(), String> {
        for p in &self.predicates {
            if let Some(def) = &p.definition {
                if def.operands.is_empty() {
                    return Err(format!(
                        "predicate `{}` has a definition with no operands",
                        p.name
                    ));
                }
            }
        }

        let mut by_name: HashMap<String, usize> = HashMap::new();
        for (i, p) in self.predicates.iter().enumerate() {
            // A repeated name resolves to the most recently added predicate, matching `state add`'s
            // append semantics. (Duplicates are legal in old files and are not rejected here.)
            by_name.insert(p.name.clone(), i);
        }

        const UNVISITED: u8 = 0;
        let mut marks = vec![UNVISITED; self.predicates.len()];
        let mut stack: Vec<usize> = Vec::new();
        let mut order: Vec<usize> = Vec::new();
        for i in 0..self.predicates.len() {
            if self.predicates[i].definition.is_some() {
                visit_definition(i, &self.predicates, &by_name, &mut marks, &mut stack, &mut order)?;
            }
        }

        let objects = object_mask(self.n);
        // `order` is post-order: every defined dependency is resolved before its dependents.
        for i in order {
            let def = self.predicates[i]
                .definition
                .clone()
                .expect("only defined predicates enter the order");
            let mut ext = match def.op {
                DefOp::AllOf => u64::MAX,
                DefOp::AnyOf => 0,
            };
            for operand in &def.operands {
                let j = *by_name
                    .get(operand.as_str())
                    .expect("the dependency walk checked every operand");
                let m = mask_of_ids(&self.predicates[j].ids);
                ext = match def.op {
                    DefOp::AllOf => ext & m,
                    DefOp::AnyOf => ext | m,
                };
            }
            ext &= objects;
            self.predicates[i].ids = ids_of_mask(ext, self.n);
        }
        Ok(())
    }

    /// Rebuild the kernel state machine from the persisted state.
    ///
    /// [`State::load`] has already resolved every definition; the masks and their definitions are
    /// copied into the vocabulary so the extension stays authoritative for this object set.
    pub fn explorer(&self) -> Explorer {
        let mut vocab = Vocabulary::with(Vec::new());
        for p in &self.predicates {
            let m = mask_of_ids(&p.ids);
            let pred = match &p.definition {
                Some(def) => Predicate::defined(p.name.clone(), m, def.clone()),
                None => Predicate::new(p.name.clone(), m),
            };
            vocab.add(pred);
        }
        let mut e = Explorer::new(self.n, vocab);
        for v in &self.votes {
            e.answer_votes(v.id, v.yes, v.no);
        }
        e
    }
}

/// Walk a definition's operands depth-first. Missing operands fail immediately; a back edge is a cycle.
fn visit_definition(
    i: usize,
    preds: &[PredSpec],
    by_name: &HashMap<String, usize>,
    marks: &mut [u8],
    stack: &mut Vec<usize>,
    order: &mut Vec<usize>,
) -> Result<(), String> {
    const ON_STACK: u8 = 1;
    const DONE: u8 = 2;
    match marks[i] {
        DONE => return Ok(()),
        ON_STACK => {
            let start = stack.iter().position(|&x| x == i).unwrap_or(0);
            let mut path: Vec<&str> =
                stack[start..].iter().map(|&x| preds[x].name.as_str()).collect();
            path.push(preds[i].name.as_str());
            return Err(format!("cyclic predicate definition: {}", path.join(" -> ")));
        }
        _ => {}
    }
    marks[i] = ON_STACK;
    stack.push(i);
    if let Some(def) = &preds[i].definition {
        for operand in &def.operands {
            let Some(&j) = by_name.get(operand.as_str()) else {
                return Err(format!(
                    "predicate `{}` is defined over unknown operand `{operand}`",
                    preds[i].name
                ));
            };
            if preds[j].definition.is_some() {
                visit_definition(j, preds, by_name, marks, stack, order)?;
            }
        }
    }
    stack.pop();
    marks[i] = DONE;
    order.push(i);
    Ok(())
}

/// Mask of the first `n` objects. Definitions only ever contain objects that exist.
fn object_mask(n: usize) -> u64 {
    if n >= 64 {
        u64::MAX
    } else if n == 0 {
        0
    } else {
        (1u64 << n) - 1
    }
}

fn mask_of_ids(ids: &[usize]) -> u64 {
    ids.iter().filter(|&&id| id < 64).fold(0, |m, &id| m | bit(id))
}

fn ids_of_mask(mask: u64, n: usize) -> Vec<usize> {
    (0..n.min(64)).filter(|&x| mask & bit(x) != 0).collect()
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
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("metaform-state-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn spec(name: &str, ids: Vec<usize>) -> PredSpec {
        PredSpec { name: name.into(), ids, definition: None }
    }

    fn defined(name: &str, op: DefOp, operands: &[&str]) -> PredSpec {
        PredSpec {
            name: name.into(),
            ids: Vec::new(),
            definition: Some(Definition {
                op,
                operands: operands.iter().map(|s| s.to_string()).collect(),
            }),
        }
    }

    fn extension(st: &State, name: &str) -> u64 {
        st.explorer()
            .vocab
            .preds
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.ext)
            .expect("predicate exists")
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("state.json");
        let st = State {
            n: 4,
            predicates: vec![spec("p", vec![0, 1])],
            votes: vec![
                VoteSpec { id: 0, yes: 3, no: 0 },
                VoteSpec { id: 2, yes: 0, no: 3 },
            ],
        };
        st.save(&path).unwrap();
        let back = State::load(&path).unwrap();
        assert_eq!(back.n, 4);
        assert_eq!(back.predicates[0].name, "p");
        assert_eq!(back.predicates[0].ids, vec![0, 1]);
        assert!(back.predicates[0].definition.is_none());
        assert_eq!(back.votes.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (a) A defined predicate keeps its definition across save/load, and the rebuilt extension is
    /// exactly the AND of its operands rather than the (deliberately stale) stored `ids`.
    #[test]
    fn defined_predicate_roundtrips_and_recomputes_its_extension() {
        let dir = temp_dir("defined-roundtrip");
        let path = dir.join("state.json");
        let st = State {
            n: 4,
            predicates: vec![
                spec("a", vec![0, 1]),
                spec("b", vec![1, 2]),
                PredSpec { ids: vec![9], ..defined("p", DefOp::AllOf, &["a", "b"]) },
            ],
            votes: vec![],
        };
        st.save(&path).unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"definition\""), "the definition must be persisted: {raw}");
        assert!(raw.contains("\"op\": \"all_of\""), "{raw}");
        assert!(raw.contains("\"operands\""), "{raw}");

        let back = State::load(&path).unwrap();
        let p = back.predicates.iter().find(|p| p.name == "p").unwrap();
        assert_eq!(p.ids, vec![1], "p = a & b = {{1}}");
        assert_eq!(p.definition.as_ref().unwrap().operands, vec!["a", "b"]);
        assert_eq!(extension(&back, "p"), extension(&back, "a") & extension(&back, "b"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (b) The stored extension is only a cache. Growing the object set and the operands makes the
    /// reloaded definition follow the operands instead of keeping the old mask.
    #[test]
    fn reloading_with_more_objects_recomputes_the_definition() {
        let dir = temp_dir("grow");
        let path = dir.join("state.json");
        std::fs::write(
            &path,
            r#"{
  "n": 2,
  "predicates": [
    {"name": "a", "ids": [0]},
    {"name": "b", "ids": [0]},
    {"name": "p", "ids": [0], "definition": {"op": "any_of", "operands": ["a", "b"]}}
  ],
  "votes": []
}"#,
        )
        .unwrap();
        let before = State::load(&path).unwrap();
        assert_eq!(before.predicates[2].ids, vec![0]);

        // The project grows to three objects and `a` now also covers object 2.
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        doc["n"] = serde_json::json!(3);
        doc["predicates"][0]["ids"] = serde_json::json!([0, 2]);
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

        let after = State::load(&path).unwrap();
        let p = after.predicates.iter().find(|p| p.name == "p").unwrap();
        assert_eq!(p.ids, vec![0, 2], "the old mask [0] must not survive the reload");
        assert_eq!(extension(&after, "p"), extension(&after, "a") | extension(&after, "b"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (c) A file written before definitions existed loads unchanged: no definition is invented and
    /// the reconstructed machine behaves exactly as one built directly in code.
    #[test]
    fn old_state_files_without_definitions_load_and_behave_identically() {
        let dir = temp_dir("legacy");
        let path = dir.join("state.json");
        std::fs::write(
            &path,
            r#"{
  "n": 4,
  "predicates": [{"name": "p", "ids": [0, 1]}],
  "votes": [{"id": 0, "yes": 3, "no": 0}, {"id": 1, "yes": 0, "no": 3}]
}"#,
        )
        .unwrap();
        let loaded = State::load(&path).unwrap();
        assert_eq!(loaded.predicates.len(), 1);
        assert!(loaded.predicates[0].definition.is_none());
        assert_eq!(loaded.predicates[0].ids, vec![0, 1]);

        let direct = State {
            n: 4,
            predicates: vec![spec("p", vec![0, 1])],
            votes: vec![
                VoteSpec { id: 0, yes: 3, no: 0 },
                VoteSpec { id: 1, yes: 0, no: 3 },
            ],
        };
        let a = loaded.explorer();
        let b = direct.explorer();
        let mut classes_a = a.classes();
        let mut classes_b = b.classes();
        classes_a.sort_unstable();
        classes_b.sort_unstable();
        assert_eq!(classes_a, classes_b);
        assert_eq!(a.status(), b.status());
        assert_eq!(colliding_pairs(&a), colliding_pairs(&b));

        // The additive field stays absent, so an old writer/reader pair sees the old shape.
        let text = serde_json::to_string(&loaded).unwrap();
        assert!(!text.contains("definition"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (d) A definition naming an operand that does not exist is an error that names the operand.
    #[test]
    fn missing_operand_on_load_is_an_error_that_names_it() {
        let dir = temp_dir("missing-operand");
        let path = dir.join("state.json");
        std::fs::write(
            &path,
            r#"{
  "n": 2,
  "predicates": [
    {"name": "a", "ids": [0]},
    {"name": "p", "ids": [0], "definition": {"op": "all_of", "operands": ["a", "ghost"]}}
  ],
  "votes": []
}"#,
        )
        .unwrap();
        let err = State::load(&path).unwrap_err();
        assert!(err.contains("ghost"), "the error must name the missing operand: {err}");
        assert!(err.contains("p"), "the error must name the definition: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// (e) Definitions that depend on each other are rejected instead of recursing forever.
    #[test]
    fn cyclic_definition_is_an_error() {
        let dir = temp_dir("cycle");
        let path = dir.join("state.json");
        std::fs::write(
            &path,
            r#"{
  "n": 2,
  "predicates": [
    {"name": "a", "ids": [], "definition": {"op": "any_of", "operands": ["b"]}},
    {"name": "b", "ids": [], "definition": {"op": "any_of", "operands": ["a"]}}
  ],
  "votes": []
}"#,
        )
        .unwrap();
        let err = State::load(&path).unwrap_err();
        assert!(err.contains("cyclic"), "the error must say the definitions cycle: {err}");
        assert!(err.contains("a -> b -> a"), "the error must show the cycle: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn definitions_resolve_in_dependency_order() {
        let dir = temp_dir("dependency-order");
        let path = dir.join("state.json");
        let st = State {
            n: 4,
            predicates: vec![
                spec("a", vec![0, 1, 2]),
                spec("b", vec![1, 2, 3]),
                defined("q", DefOp::AllOf, &["a", "b"]),
                defined("p", DefOp::AnyOf, &["q", "b"]),
            ],
            votes: vec![],
        };
        st.save(&path).unwrap();
        let back = State::load(&path).unwrap();
        let p = back.predicates.iter().find(|p| p.name == "p").unwrap();
        assert_eq!(p.ids, vec![1, 2, 3], "p = (a & b) | b = b");
        assert_eq!(extension(&back, "q"), extension(&back, "a") & extension(&back, "b"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn explorer_rebuilds_classes_and_status() {
        let st = State {
            n: 4,
            predicates: vec![spec("p", vec![0, 1])],
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
            predicates: vec![spec("p", vec![0])],
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

