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
use metaform::{Status};
use metaform_runtime::run::{fingerprint, run, Step};
use metaform_runtime::{splits, status_with, validate, Operator, SmeAnalogizer, VsaRetriever};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Insufficient => "INSUFFICIENT",
        Status::Identified => "IDENTIFIED",
        Status::Undetermined => "UNDETERMINED",
    }
}

fn print_step(step: &Step) {
    if step.adopted {
        println!(
            "  step {}: {} proposed {} ({} of {} pairs), {} -> {}",
            step.index,
            step.operator,
            step.proposal,
            step.splits,
            step.colliding_before,
            status_name(step.status_before),
            status_name(step.status_after)
        );
    } else {
        println!(
            "  step {}: nothing separated the {} colliding pair(s); stopped",
            step.index, step.colliding_before
        );
    }
}

fn record_line(state_hash: u64, step: &Step) -> String {
    format!(
        "{:016x}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{}\t{}",
        state_hash,
        step.index,
        step.operator,
        step.proposal,
        step.splits,
        step.colliding_before,
        step.adopted as u8,
        step.cost,
        status_name(step.status_before),
        status_name(step.status_after)
    )
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

    // Every subcommand needs a state; only the two proposal subcommands need a response file.
    let Some(state_path) = arg(rest, "--state") else { usage() };
    let out = arg(rest, "--out");
    if command != "run" && out.is_none() {
        usage();
    }
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
        "run" => {
            let mut ctx = ctx.clone();
            let mut operators: Vec<Box<dyn Operator>> = Vec::new();
            if let Some(systems_path) = arg(rest, "--systems").or(state.systems.clone()) {
                let systems = match Systems::read(&systems_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("cannot read the systems: {}", e);
                        std::process::exit(2);
                    }
                };
                operators.push(Box::new(SmeAnalogizer {
                    base: systems.base,
                    target: systems.target,
                    readings: systems.readings,
                }));
            }
            operators.push(Box::new(VsaRetriever {
                dim: arg(rest, "--dim").and_then(|v| v.parse().ok()).unwrap_or(2048),
                seed: arg(rest, "--seed").and_then(|v| v.parse().ok()).unwrap_or(7),
                max_arity: arg(rest, "--max-arity").and_then(|v| v.parse().ok()).unwrap_or(2),
            }));

            let max_steps: usize = arg(rest, "--max-steps").and_then(|v| v.parse().ok()).unwrap_or(8);
            let state_hash = fingerprint(&ctx);
            let steps = run(&mut ctx, &operators, max_steps);

            println!("run {} (state {:016x})", command, state_hash);
            for step in &steps {
                print_step(step);
            }
            let final_status = status_with(&ctx, &[]);
            println!(
                "final: {} predicates, status {}",
                ctx.vocabulary.len(),
                status_name(final_status)
            );

            if let Some(path) = arg(rest, "--record") {
                let mut body = String::new();
                for step in &steps {
                    body.push_str(&record_line(state_hash, step));
                    body.push('\n');
                }
                use std::io::Write;
                match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(body.as_bytes()) {
                            eprintln!("cannot write the record: {}", e);
                            std::process::exit(2);
                        }
                        println!("steps recorded in {}", path);
                    }
                    Err(e) => {
                        eprintln!("cannot open {}: {}", path, e);
                        std::process::exit(2);
                    }
                }
            }
            std::process::exit(0);
        }
        _ => usage(),
    };

    let out = out.expect("checked above");
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
