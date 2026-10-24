//! `metaform axes list` -- what the current vocabulary leaves open.
//!
//! The claim this answers is the closed-world one: the user's enumeration is a sample drawn from a
//! set they never stated, and a model that treats the enumeration as the set has an empty
//! complement, so there is nothing to be surprised by. To see a complement there has to be a grid,
//! and the grid that already exists is the **equivalence class**: under the current vocabulary two
//! objects that agree on every predicate are the same place.
//!
//! Three numbers, all computable without a model:
//!
//! - **legal axes**: candidate readings that agree with the evidence (they contain every positive and
//!   no negative), so the kernel would admit them;
//! - **coverage**: legal axes whose extension differs from every predicate already in force -- the
//!   candidates for "what the user did not say";
//! - **equivalence**: the rest -- the same distinctions, reworded.
//!
//! What it does **not** compute: the axis-equivalence rate of `CHARTER.md` 6.10 over the *stated
//! elements*, because a stated element is an object while an axis reads a set. That layer needs the
//! notion of a concept family, which the repository does not have yet (backlog B16). The number is
//! left out rather than approximated.

use std::path::PathBuf;

use metaform_kernel::{bit, Mask};
use metaform_store::state::State;

/// One candidate reading, with the extension it would have.
struct Axis {
    label: String,
    ext: Mask,
}

fn mask_of(ids: &[usize]) -> Mask {
    ids.iter().fold(0, |acc, &id| acc | bit(id))
}

/// The strong labels: an object is positive when more votes say yes than no, negative when the other
/// way, and undetermined when the votes are even.
struct Evidence {
    pos: Mask,
    neg: Mask,
    undecided: Vec<usize>,
}

fn evidence_of(state: &State) -> Evidence {
    let mut pos = 0;
    let mut neg = 0;
    let mut undecided = Vec::new();
    let mut votes: Vec<(usize, u8, u8)> = state.votes.iter().map(|v| (v.id, v.yes, v.no)).collect();
    votes.sort_by_key(|(id, _, _)| *id);
    for (id, yes, no) in votes {
        if yes > no {
            pos |= bit(id);
        } else if no > yes {
            neg |= bit(id);
        } else {
            undecided.push(id);
        }
    }
    Evidence { pos, neg, undecided }
}

/// The candidate readings, generated deterministically from the vocabulary in force:
///
/// - each predicate on its own;
/// - the conjunction of each pair of predicates;
/// - the complement of each predicate.
///
/// Nothing here is random and nothing here comes from a model: the point is to enumerate the grid the
/// current vocabulary already implies, so that "what else could have been said" is a countable thing.
fn axes_of(state: &State, max_arity: usize) -> Vec<Axis> {
    let predicates: Vec<(String, Mask)> = state
        .predicates
        .iter()
        .map(|p| (p.name.clone(), mask_of(&p.ids)))
        .collect();
    let universe: Mask = if state.n >= 64 { u64::MAX } else { (1u64 << state.n) - 1 };
    let mut out = Vec::new();
    for (name, m) in &predicates {
        out.push(Axis { label: name.clone(), ext: *m });
    }
    if max_arity >= 2 {
        for i in 0..predicates.len() {
            for j in (i + 1)..predicates.len() {
                out.push(Axis {
                    label: format!("{} & {}", predicates[i].0, predicates[j].0),
                    ext: predicates[i].1 & predicates[j].1,
                });
            }
        }
    }
    for (name, m) in &predicates {
        out.push(Axis {
            label: format!("not {}", name),
            ext: universe & !*m,
        });
    }
    out
}

fn ids_of(mask: Mask, n: usize) -> Vec<usize> {
    (0..n).filter(|&i| mask & bit(i) != 0).collect()
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The grid: the equivalence classes the current vocabulary induces, and what is known about each.
fn classes_json(state: &State, evidence: &Evidence) -> Vec<String> {
    let mut classes: Vec<Mask> = Vec::new();
    for p in &state.predicates {
        let m = mask_of(&p.ids);
        match classes.iter_mut().find(|c| **c == m) {
            Some(slot) => *slot = m,
            None => classes.push(m),
        }
    }
    let mut out = Vec::new();
    for c in classes {
        let ids = ids_of(c, state.n);
        let labelled: Vec<usize> = ids
            .iter()
            .copied()
            .filter(|&i| evidence.pos & bit(i) != 0 || evidence.neg & bit(i) != 0)
            .collect();
        let empty = labelled.is_empty();
        out.push(format!(
            "{{\"objects\":[{}],\"labelled\":[{}],\"empty\":{}}}",
            ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            labelled.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            empty
        ));
    }
    out
}

/// The command. Returns the process exit code.
pub fn cmd(args: &[String]) -> i32 {
    let Some(sub) = args.get(1).map(String::as_str) else {
        eprintln!("usage: metaform axes list [--project .] [--max-arity 2]");
        return 2;
    };
    if sub != "list" {
        eprintln!("metaform axes: unknown sub-command `{sub}`; expected list");
        return 2;
    }
    let root = PathBuf::from(
        args.iter()
            .position(|a| a == "--project")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| ".".to_string()),
    );
    let max_arity: usize = args
        .iter()
        .position(|a| a == "--max-arity")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);

    let state_path = root.join(".metaform").join("state.json");
    let state = match State::load(&state_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("metaform axes list: cannot read {}: {e}", state_path.display());
            return 2;
        }
    };
    let evidence = evidence_of(&state);
    let axes = axes_of(&state, max_arity);
    let in_force: Vec<Mask> = state.predicates.iter().map(|p| mask_of(&p.ids)).collect();

    let mut legal = 0usize;
    let mut coverage = 0usize;
    let mut equivalence = 0usize;
    let mut rows: Vec<String> = Vec::new();
    let universe: Mask = if state.n >= 64 { u64::MAX } else { (1u64 << state.n) - 1 };
    for axis in &axes {
        if axis.ext == 0 || axis.ext == universe {
            continue;
        }
        if axis.ext & evidence.neg != 0 || evidence.pos & !axis.ext != 0 {
            continue;
        }
        legal += 1;
        let same_as_existing = in_force.iter().any(|m| *m == axis.ext);
        if same_as_existing {
            equivalence += 1;
        } else {
            coverage += 1;
        }
        rows.push(format!(
            "{{\"axis\":{},\"objects\":[{}],\"new\":{}}}",
            json_str(&axis.label),
            ids_of(axis.ext, state.n)
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(","),
            !same_as_existing
        ));
    }

    println!(
        "{{\"objects\":{},\"predicates\":{},\"legal_axes\":{},\"coverage\":{},\"equivalence\":{},\"classes\":[{}],\"axes\":[{}],\"note\":\"coverage counts legal axes whose extension differs from every predicate in force; the axis-equivalence rate over stated *elements* needs concept families and is not computed here\"}}",
        state.n,
        state.predicates.len(),
        legal,
        coverage,
        equivalence,
        classes_json(&state, &evidence).join(","),
        rows.join(",")
    );
    0
}

