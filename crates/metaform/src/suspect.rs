//! `metaform suspect list` -- which labels are too thin to be treated as settled.
//!
//! The state command already answers two questions: which objects are *undetermined* (the vote gap is
//! below the threshold) and which are *unprobed* (never asked). Neither catches the case that cost the
//! most on 2026-10-24: **a label that was asked once, decided by a wide margin, and wrong.**
//!
//! In the `noisy` family the corrupted object carried a clean 3:0 against it -- stronger evidence than
//! a correctly labelled object carries -- and nine candidates built on that evidence were admitted
//! before anyone noticed. The gate that caught them did not compare the candidate with the evidence;
//! it *re-asked*, and the answer flipped the vote to 3:3, which the vote gap then refuses to call.
//!
//! So the thing a project needs to be told is not "this is wrong" (it cannot know) but **"this label
//! rests on a single round"** -- which is a fact about the votes, computable without a model, and a
//! reason to ask again rather than to act.
//!
//! `suspect list` reports them, and it is deliberately a list rather than a verdict: the tool cannot
//! re-ask, because the oracle is not in the tool.

use std::path::PathBuf;

use metaform_store::state::State;

/// One label that a single round decided.
struct Suspect {
    id: usize,
    yes: u8,
    no: u8,
    gap: i16,
    why: &'static str,
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

/// The labels that a single round decided, in the sense that the total number of votes is no more than
/// one oracle visit. A second visit would either widen the margin or split it, and only the second
/// outcome is evidence of a bad label -- which is exactly what a thin margin cannot tell you.
fn suspects(state: &State, theta: u8, one_visit: u8) -> Vec<Suspect> {
    let mut out = Vec::new();
    for v in &state.votes {
        let gap = i16::from(v.yes) - i16::from(v.no);
        let total = u16::from(v.yes) + u16::from(v.no);
        let decided = gap.unsigned_abs() >= u16::from(theta);
        if total <= u16::from(one_visit) && decided {
            out.push(Suspect {
                id: v.id,
                yes: v.yes,
                no: v.no,
                gap,
                why: "one visit decided it: a second visit can widen the margin or split it, and only the split is evidence",
            });
        }
    }
    out.sort_by_key(|s| s.id);
    out
}

/// The command. Returns the process exit code.
pub fn cmd(args: &[String]) -> i32 {
    let Some(sub) = args.get(1).map(String::as_str) else {
        eprintln!("usage: metaform suspect list [--project .] [--theta 3] [--one-visit 3]");
        return 2;
    };
    if sub != "list" {
        eprintln!("metaform suspect: unknown sub-command `{sub}`; expected list");
        return 2;
    }
    let arg = |name: &str, default: &str| -> String {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.to_string())
    };
    let root = PathBuf::from(arg("--project", "."));
    let theta: u8 = arg("--theta", "3").parse().unwrap_or(3);
    let one_visit: u8 = arg("--one-visit", "3").parse().unwrap_or(3);

    let state_path = root.join(".metaform").join("state.json");
    let state = match State::load(&state_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("metaform suspect list: cannot read {}: {e}", state_path.display());
            return 2;
        }
    };
    let found = suspects(&state, theta, one_visit);
    let rows: Vec<String> = found
        .iter()
        .map(|s| {
            format!(
                "{{\"id\":{},\"yes\":{},\"no\":{},\"gap\":{},\"why\":{}}}",
                s.id,
                s.yes,
                s.no,
                s.gap,
                json_str(s.why)
            )
        })
        .collect();
    println!(
        "{{\"theta\":{},\"one_visit\":{},\"labelled\":{},\"suspects\":[{}],\"note\":\"a suspect is a label a single round decided; the tool cannot re-ask, because the oracle is not in the tool\"}}",
        theta,
        one_visit,
        state.votes.len(),
        rows.join(",")
    );
    0
}
