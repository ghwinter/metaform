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
use metaform_runtime::experience::{class_of, Experience};
use metaform_runtime::file_proposer::FileProposer;
use metaform_runtime::run::{fingerprint, run, Step};
use metaform_runtime::{splits, status_with, validate, Operator, SmeAnalogizer, VsaRetriever};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

/// A flag without a value. `arg` looks at the *next* token, so a bare `--probe` at the end of a
/// command line would otherwise be invisible.
fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Insufficient => "INSUFFICIENT",
        Status::Identified => "IDENTIFIED",
        Status::Undetermined => "UNDETERMINED",
    }
}

fn print_step(step: &Step) {
    match step.adopted() {
        Some(offer) => {
            println!(
                "  step {}: {} proposed {} ({} of {} pairs), {} -> {}",
                step.index,
                offer.operator,
                offer.name,
                offer.splits,
                step.colliding_before,
                status_name(step.status_before),
                status_name(step.status_after)
            );
        }
        None => println!(
            "  step {}: nothing separated the {} colliding pair(s); stopped",
            step.index, step.colliding_before
        ),
    }
    if let Some(conflict) = &step.conflict {
        println!("      conflict: {}", conflict);
    }
    for offer in &step.offers {
        if !offer.adopted {
            println!(
                "      offered {} {}: {} pair(s), cost {:.3} (not adopted){}",
                offer.operator,
                offer.name,
                offer.splits,
                offer.cost,
                if offer.note.is_empty() { String::new() } else { format!(" -- {}", offer.note) }
            );
        }
    }
}

/// One record line per offer: what the operator proposed, whether it won, and the state class.
/// One record line per offer: what the operator proposed, whether it won, and the state class.
fn record_lines(state_hash: u64, step: &Step, ctx: &metaform_runtime::Context) -> Vec<String> {
    let class = metaform_runtime::record::Class {
        objects: ctx.objects,
        vocabulary: step.vocabulary_before,
        labelled: (ctx.pos.count_ones() + ctx.neg.count_ones()) as usize,
        colliding: step.colliding_before,
    };
    step.offers
        .iter()
        .map(|offer| {
            metaform_runtime::record::Record {
                fingerprint: state_hash,
                step: step.index,
                operator: offer.operator.clone(),
                name: offer.name.clone(),
                splits: offer.splits,
                colliding: step.colliding_before,
                adopted: offer.adopted,
                cost: offer.cost as f64,
                status_before: status_name(step.status_before).to_string(),
                status_after: status_name(step.status_after).to_string(),
                class,
                conflict: step.conflict.is_some(),
            }
            .to_line()
        })
        .collect()
}
fn usage() -> ! {
    eprintln!(
        "usage:\n  metaform-runtime retrieve  --state <file> --out <response.json> [--dim N] [--seed N] [--limit N]\n  metaform-runtime analogize --state <file> --systems <file> --out <response.json> [--limit N]"
    );
    std::process::exit(2);
}

fn write_response(
    path: &str,
    request_id: &str,
    objects: usize,
    candidates: &[metaform_runtime::Candidate],
) -> std::io::Result<()> {
    let ids: Vec<Vec<usize>> = candidates
        .iter()
        .map(|c| metaform::propose::ids_of(c.extension, objects))
        .collect();
    let warrants: Vec<String> = candidates.iter().map(|c| c.warrant.join("; ")).collect();
    let falsifiers: Vec<String> = candidates.iter().map(|c| c.falsifier.join("; ")).collect();
    let proposals: Vec<metaform::propose::Proposal<'_>> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| metaform::propose::Proposal {
            name: &c.name,
            ids: &ids[i],
            warrant: Some(warrants[i].as_str()),
            falsifier: Some(falsifiers[i].as_str()),
        })
        .collect();
    std::fs::write(path, metaform::propose::response_file(request_id, &proposals))
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1).map(|s| s.as_str()) else { usage() };
    let rest = &args[2..];

    // Reject unknown flags instead of ignoring them: a silently ignored flag is how a stale binary
    // spent a whole experiment running without the proposer it was told to use.
    let known = [
        "--state", "--out", "--systems", "--proposals", "--record", "--use-record", "--max-steps", "--probe",
        "--dim", "--seed", "--limit", "--max-arity",
    ];
    for arg in rest {
        if arg.starts_with("--") && !known.contains(&arg.as_str()) {
            eprintln!("unknown flag: {}", arg);
            usage();
        }
    }

    // Every subcommand but `experience` needs a state; the two proposal subcommands also need a
    // response file.
    let state = match arg(rest, "--state") {
        Some(path) => match State::read(&path) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("cannot read the state: {}", e);
                std::process::exit(2);
            }
        },
        None => None,
    };
    if state.is_none() && command != "experience" {
        usage();
    }
    let out = arg(rest, "--out");
    if matches!(command, "retrieve" | "analogize") && out.is_none() {
        usage();
    }
    let ctx = state.as_ref().map(|s| s.context()).unwrap_or_else(|| {
        metaform_runtime::context(0, Default::default(), Vec::new())
    });
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
            let Some(systems_path) = arg(rest, "--systems").or_else(|| state.as_ref().and_then(|s| s.systems.clone())) else {
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
        "experience" => {
            let Some(record) = arg(rest, "--record") else {
                eprintln!("experience needs --record <file>");
                std::process::exit(2);
            };
            let experience = match Experience::load(std::path::Path::new(&record)) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("cannot read {}: {}", record, e);
                    std::process::exit(2);
                }
            };
            println!("class (objects, vocabulary, labelled, colliding)  operator  attempts  adoptions  mean cost");
            for (class, operator, tally) in experience.rows() {
                println!(
                    "({}, {}, {}, {})  {}  {}  {}  {:.3}",
                    class.objects,
                    class.vocabulary,
                    class.labelled,
                    class.colliding,
                    operator,
                    tally.attempts,
                    tally.adoptions,
                    tally.mean_cost()
                );
            }
            if !experience.conflicts.is_empty() {
                println!();
                println!("label conflicts by class");
                for (class, count) in &experience.conflicts {
                    println!(
                        "({}, {}, {}, {})  {} step(s) with a doubtful label",
                        class.objects, class.vocabulary, class.labelled, class.colliding, count
                    );
                }
            }
            let Some(state_ref) = state.as_ref() else {
                std::process::exit(0);
            };
            let ctx = state_ref.context();
            let class = class_of(&ctx);
            match experience.suggest(&class) {
                Some(name) => println!(
                    "this state is class ({}, {}, {}, {}); suggestion: {}",
                    class.objects, class.vocabulary, class.labelled, class.colliding, name
                ),
                None => println!(
                    "this state is class ({}, {}, {}, {}); suggestion: none, the class has not been seen",
                    class.objects, class.vocabulary, class.labelled, class.colliding
                ),
            }
            std::process::exit(0);
        }
        "run" => {
            let mut ctx = state.as_ref().map(|s| s.context()).expect("run needs a state");
            let mut operators: Vec<Box<dyn Operator>> = Vec::new();
            if let Some(systems_path) = arg(rest, "--systems").or_else(|| state.as_ref().and_then(|s| s.systems.clone())) {
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
            if let Some(path) = arg(rest, "--proposals") {
                match FileProposer::load(std::path::Path::new(&path)) {
                    Ok(p) => {
                        println!("proposals: {} line(s) from {}", p.lines, path);
                        operators.push(Box::new(p));
                    }
                    Err(e) => {
                        eprintln!("cannot read {}: {}", path, e);
                        std::process::exit(2);
                    }
                }
            }
            if flag(rest, "--probe") {
                operators.push(Box::new(metaform_runtime::probe::ProbeLabels::new(2)));
            }
            operators.push(Box::new(VsaRetriever {
                dim: arg(rest, "--dim").and_then(|v| v.parse().ok()).unwrap_or(2048),
                seed: arg(rest, "--seed").and_then(|v| v.parse().ok()).unwrap_or(7),
                max_arity: arg(rest, "--max-arity").and_then(|v| v.parse().ok()).unwrap_or(2),
            }));

            // If a record is given, the operator that has adopted most often in this class goes
            // first. The order is the only thing experience changes: every operator is still
            // asked, and the controller still decides by pairs separated.
            if let Some(record) = arg(rest, "--use-record") {
                let experience = match Experience::load(std::path::Path::new(&record)) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("cannot read {}: {}", record, e);
                        std::process::exit(2);
                    }
                };
                let class = class_of(&ctx);
                match experience.suggest(&class) {
                    Some(name) => {
                        println!("experience: {} has adopted most often in this class", name);
                        operators.sort_by_key(|op| if op.name() == name { 0 } else { 1 });
                    }
                    None => println!("experience: this class has not been seen; default order"),
                }
            }

            let max_steps: usize = arg(rest, "--max-steps").and_then(|v| v.parse().ok()).unwrap_or(8);
            let budget: usize = arg(rest, "--budget").and_then(|v| v.parse().ok()).unwrap_or(4);
            let state_hash = fingerprint(&ctx);
            let steps = run(&mut ctx, &operators, max_steps, budget);

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
                // A new file starts with the format version; an existing one already has it.
                let fresh = !std::path::Path::new(&path).exists();
                let mut body = String::new();
                if fresh {
                    body.push_str(metaform_runtime::record::VERSION);
                    body.push('\n');
                }
                for step in &steps {
                    for line in record_lines(state_hash, step, &ctx) {
                        body.push_str(&line);
                        body.push('\n');
                    }
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
    if let Err(e) = write_response(&out, &state.as_ref().map(|s| s.request_id.clone()).unwrap_or_default(), state.as_ref().map(|s| s.objects).unwrap_or(0), &result.candidates) {
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
