//! The inductive learner as a tool: evidence plus background knowledge in, a candidate extension out.
//!
//! The engine never learns. It decides whether a candidate is any good, and it can only do that if
//! somebody proposes one. This binary is that somebody, for the case where the rule to be found is a
//! conjunction of known predicates: it takes the background facts, the labelled positives and
//! negatives, and returns the extension of the first clause that covers every positive and no
//! negative.
//!
//! Why a tool and not a library call: the point of the comparison is what a *proposer* contributes.
//! A proposer that returns a set of object ids can be handed to `metaform propose` and judged by the
//! kernel, exactly like a model's answer, so the arms differ only in where the candidate came from.
//!
//! Input (JSON, on stdin or in `--input`):
//!
//! ```json
//! {"signature": [{"name": "a0", "arity": 1}],
//!  "facts": [["a0", [2]], ["a1", [3]]],
//!  "positives": [[3], [5]],
//!  "negatives": [[1], [4]],
//!  "domain": [0, 1, 2, 3, 4, 5],
//!  "target": "target", "max_body": 3}
//! ```
//!
//! Output (JSON): `{"learned": true, "clause": {...}, "extension": [[2], [3]]}` or
//! `{"learned": false}` when no clause within the bound covers the positives and spares the
//! negatives. A failure to learn is a normal answer, not an error: it is the "this language cannot
//! say it" case, and the kernel's judgement does not apply to it.
//!
//! Usage: ilp_learn [--input <file>] [--max-body <n>]

use std::io::Read;

use metaform_ilp::{extension, learn, Atom, Background, Signature, Term};
use serde_json::Value;

fn atom_from_json(value: &Value, pred: &str, what: &str) -> Result<Atom, String> {
    let args = value
        .as_array()
        .ok_or_else(|| format!("{what} must be an array of argument lists"))?;
    let mut terms = Vec::with_capacity(args.len());
    for arg in args {
        let id = arg
            .as_u64()
            .ok_or_else(|| format!("{what}: arguments must be object ids"))? as u32;
        terms.push(Term::Const(id));
    }
    Ok(Atom { pred: pred.to_string(), args: terms })
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut input_path: Option<String> = None;
    let mut max_body: usize = 3;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input_path = args.next(),
            "--max-body" => {
                max_body = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(max_body)
            }
            other => {
                eprintln!("ilp_learn: unknown argument `{other}`");
                std::process::exit(2);
            }
        }
    }

    let mut text = String::new();
    match input_path {
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(t) => text = t,
            Err(e) => {
                eprintln!("ilp_learn: cannot read {path}: {e}");
                std::process::exit(2);
            }
        },
        None => {
            if let Err(e) = std::io::stdin().read_to_string(&mut text) {
                eprintln!("ilp_learn: cannot read stdin: {e}");
                std::process::exit(2);
            }
        }
    }

    let root: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("ilp_learn: input is not JSON: {e}");
            std::process::exit(2);
        }
    };

    let mut bg = Background::new(Vec::new());
    if let Some(sigs) = root.get("signature").and_then(Value::as_array) {
        for sig in sigs {
            let name = sig.get("name").and_then(Value::as_str).unwrap_or("");
            let arity = sig.get("arity").and_then(Value::as_u64).unwrap_or(1) as usize;
            bg.signature.push(Signature { name: name.to_string(), arity });
        }
    }
    if let Some(facts) = root.get("facts").and_then(Value::as_array) {
        for fact in facts {
            let Some(arr) = fact.as_array() else {
                eprintln!("ilp_learn: a fact must be an array [predicate, [ids]]");
                std::process::exit(2);
            };
            let pred = arr.first().and_then(Value::as_str).unwrap_or("");
            let mut ids = Vec::new();
            if let Some(args) = arr.get(1).and_then(Value::as_array) {
                for a in args {
                    ids.push(a.as_u64().unwrap_or(0) as u32);
                }
            }
            // `["a0", [0, 1, 4]]` means "the one-place predicate a0 holds of objects 0, 1 and 4",
            // which is three ground facts, not one fact with three arguments.
            for id in &ids {
                bg.fact(pred, &[*id]);
            }
        }
    }
    // A signature that the caller did not declare still has to be available as a body literal, or
    // the learner has nothing to build a body out of.
    for fact in &bg.facts {
        if !bg.signature.iter().any(|s| s.name == fact.pred) {
            bg.signature.push(Signature { name: fact.pred.clone(), arity: fact.args.len() });
        }
    }

    let target = root.get("target").and_then(Value::as_str).unwrap_or("target");
    let positives: Vec<Atom> = root
        .get("positives")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| atom_from_json(v, target, "positives").ok())
                .collect()
        })
        .unwrap_or_default();
    let negatives: Vec<Atom> = root
        .get("negatives")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| atom_from_json(v, target, "negatives").ok())
                .collect()
        })
        .unwrap_or_default();
    let domain: Vec<u32> = root
        .get("domain")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|v| v.as_u64().map(|x| x as u32)).collect())
        .unwrap_or_default();
    let clause = learn(target, 1, &positives, &negatives, &bg, max_body, 1);
    let Some(clause) = clause else {
        println!("{{\"learned\":false}}");
        return;
    };
    let ext = extension(&clause, &bg, &domain);
    let body: Vec<String> = clause
        .body
        .iter()
        .map(|a| {
            let args: Vec<String> = a
                .args
                .iter()
                .map(|t| match t {
                    Term::Var(v) => format!("V{v}"),
                    Term::Const(c) => c.to_string(),
                })
                .collect();
            format!("{}({})", a.pred, args.join(","))
        })
        .collect();
    let ext_json: Vec<String> = ext
        .iter()
        .map(|args| format!("[{}]", args.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")))
        .collect();
    println!(
        "{{\"learned\":true,\"body\":[{}],\"extension\":[{}]}}",
        body.iter().map(|b| format!("\"{b}\"")).collect::<Vec<_>>().join(","),
        ext_json.join(",")
    );
}
