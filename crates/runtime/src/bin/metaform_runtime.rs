//! The single entry point for the proposal components.
//!
//!   metaform-runtime retrieve  --state <file> --out <response.json> [--dim N] [--seed N] [--limit N]
//!   metaform-runtime analogize --state <file> --systems <file> --out <response.json> [--limit N]
//!
//! Both subcommands read a state in the line format (`crates/runtime/src/state.rs`), run one
//! component, and write a `metaform.propose/1` response file that the engine can validate:
//!
//!   metaform propose --request <req.json> --response <response.json>
//!
//! What the components do not do is decide. The response says what was proposed; whether it is well
//! formed and whether it separates anything is for the kernel to say.

use metaform_runtime::state::{State, Systems};
use metaform_runtime::{splits, validate, Operator, SmeAnalogizer, VsaRetriever};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  metaform-runtime retrieve  --state <file> --out <response.json> [--dim N] [--seed N] [--limit N]\n  metaform-runtime analogize --state <file> --systems <file> --out <response.json> [--limit N]"
    );
    std::process::exit(2);
}

/// Escapes a string for a JSON field.
fn json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_response(
    path: &str,
    request_id: &str,
    candidates: &[metaform_runtime::Candidate],
) -> std::io::Result<()> {
    let mut body = String::new();
    body.push_str("{\"protocol\":\"metaform.propose/1\",\"request_id\":");
    body.push_str(&json(request_id));
    body.push_str(",\"candidates\":[");
    for (i, c) in candidates.iter().enumerate() {
        if i > 0 {
            body.push(',');
        }
        let ids: Vec<String> = (0..64)
            .filter(|o| (c.extension >> o) & 1 == 1)
            .map(|o| o.to_string())
            .collect();
        body.push_str(&format!(
            "{{\"name\":{},\"ids\":[{}],\"warrant\":{},\"falsifier\":{}}}",
            json(&c.name),
            ids.join(","),
            json(&c.warrant.join("; ")),
            json(&c.falsifier.join("; "))
        ));
    }
    body.push_str("],\"refused\":null}\n");
    std::fs::write(path, body)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1).map(|s| s.as_str()) else { usage() };
    let rest = &args[2..];

    let (Some(state_path), Some(out)) = (arg(rest, "--state"), arg(rest, "--out")) else { usage() };
    let state = match State::read(&state_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read the state: {}", e);
            std::process::exit(2);
        }
    };
    let ctx = state.context();
    let limit: usize = arg(rest, "--limit").and_then(|v| v.parse().ok()).unwrap_or(3);

    let result = match command {
        "retrieve" => {
            let op = VsaRetriever {
                dim: arg(rest, "--dim").and_then(|v| v.parse().ok()).unwrap_or(2048),
                seed: arg(rest, "--seed").and_then(|v| v.parse().ok()).unwrap_or(7),
                max_arity: arg(rest, "--max-arity").and_then(|v| v.parse().ok()).unwrap_or(2),
            };
            op.execute(&ctx, limit)
        }
        "analogize" => {
            let Some(systems_path) = arg(rest, "--systems").or(state.systems.clone()) else {
                eprintln!("analogize needs --systems or a `systems` line in the state");
                std::process::exit(2);
            };
            let systems = match Systems::read(&systems_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("cannot read the systems: {}", e);
                    std::process::exit(2);
                }
            };
            let op = SmeAnalogizer {
                base: systems.base,
                target: systems.target,
                readings: systems.readings,
            };
            op.execute(&ctx, limit)
        }
        _ => usage(),
    };

    if let Err(e) = write_response(&out, &state.request_id, &result.candidates) {
        eprintln!("cannot write {}: {}", out, e);
        std::process::exit(2);
    }

    // The human-readable part goes to stdout; the response file stays machine-readable.
    println!(
        "{}: {:?}, {} candidate(s), cost {:.3}",
        command,
        result.status,
        result.candidates.len(),
        result.cost
    );
    for line in &result.evidence {
        println!("  evidence: {}", line);
    }
    for line in &result.assumptions {
        println!("  assumption: {}", line);
    }
    for line in &result.unresolved {
        println!("  unresolved: {}", line);
    }
    let verdict = validate(&ctx, &result.candidates);
    for c in &verdict.accepted {
        println!(
            "  proposed {}: {} (separates {} colliding pair(s))",
            c.name,
            c.detail,
            splits(&ctx, c.extension)
        );
    }
    for (c, why) in &verdict.rejected {
        println!("  refused {}: {}", c.name, why);
    }
    println!("  response written to {}", out);
}
