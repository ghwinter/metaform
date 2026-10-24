//! `metaform` — the command-line workbench for query-driven representation change.
//!
//! Reads commands from a script file (or stdin) and builds and observes the state machine step by step.
//! Its purpose is not to run benchmarks but to **lay the current scenario out while thinking**:
//! what is missing now, which pair is colliding, how many pairs a candidate predicate would split.
//!
//! Commands:
//! ```text
//! objects 21                     # new scenario (objects 0..20)
//! add <name> <id...>             # add a predicate to the vocabulary; its extension is a list of ids
//! pos <id...>   neg <id...>      # label positives / negatives
//! ask                            # the next object suggested for a question
//! status                         # current status + number of open classes
//! explain                        # print all equivalence classes, marking colliding pairs
//! score <name> <id...>           # do not add it, just report how many colliding pairs it splits
//! capacity                       # 2^(number of classes)
//! ```
//! Text after `#` on a line is a comment.
//!
//! Protocol boundary: `metaform propose --request <request.json> --response <response.json>` validates
//! the predicates an external model proposed under `metaform.propose/1` and prints one line of JSON
//! without writing any file or touching the project state.
//!
//! `metaform apply --project <dir> --request <request.json> --response <response.json>` runs that same
//! validation and then adopts the accepted candidates, writing `.metaform/state.json` once at the
//! end. It skips a candidate whose name already exists, reports every candidate it did not adopt,
//! and leaves the state unchanged when the response is rejected as a whole or an adoption fails.
//! Every adopted candidate that carried a held-out `predict` is recorded in
//! `.metaform/predictions.tsv` (L11), and `metaform check-predictions --project <dir>` scores those
//! commitments once the state has a strong label for the predicted object.
//!
//! `metaform suggest --project . [--max-arity 2] [--max 20] [--emit-request [--out <path>]]` is the
//! read-only proof step for an `INSUFFICIENT` state: it enumerates the Boolean definitions the
//! vocabulary invites, counts how many it tried and how many colliding pairs they separate, and
//! reports that the answer is none because every colliding pair shares a signature. It never adds a
//! predicate. With `--emit-request` it skips the enumeration and prints a ready-to-fill
//! `metaform.propose/1` request instead (or `{"status":"<STATUS>","request":null}` when no new atom
//! is needed), so a proposer can answer without inventing the request shape.
//!
//! `metaform explore --project . [--max-questions N] [--laws <path>] [--emit-questions <path>] [--out <path>]`
//! is the read-only attribute exploration: it walks the closed sets of the law theory, confirms an
//! implication when the labelled evidence implies it and no object contradicts it, records a
//! refutation with its counterexample object, and records an open question when no evidence object
//! matches the premise. The initial theory is `.metaform/laws.tsv` (or `--laws`), whose `kind`
//! column separates laws from recorded refutations: a premise decided by either is not asked again,
//! and a recorded refutation is reported in `refuted` with its source. It prints one deterministic
//! JSON line (or writes it to `--out`) and never saves the state; `--emit-questions` writes the open
//! questions for `answer` and suppresses the stdout line.
//!
//! `metaform coverage [--project .] [--format json|text] [<table.tsv>]` is the read-only
//! goal-coverage check: a TSV of work items against required properties becomes the missing cells,
//! the conflicts predicted by implications supported by at least two rows, the rows with identical
//! answers, and the blocking requirements still unanswered. A requirement named with a trailing
//! `!` is blocking. It never writes a file.
//!
//! `metaform translate --input <spec.json|-> [--max-classes N]` is the read-only judgment on a
//! *change of vocabulary*: it takes two theories and a map on their objects, and reports whether the
//! change is `lossy` (with a witness: a merged pair an old atom separates, or an old atom the new
//! vocabulary can no longer define), `wording_only` (faithful, and every new definable set was
//! already definable), or `genuine` (faithful, with a count of the sets that were not). Adding an
//! atom — what `Op::Represent` does in place — is the case where the object map is the identity, so
//! the kernel's representation step is one row of a table whose other rows it cannot yet express.
//! With `pos` / `neg` it also reports `collapsed`: a fibre holding one positive and one negative
//! object, which loses no definable set and destroys the only proof that the vocabulary was
//! insufficient.
//!
//! `metaform answer --project . --questions <q.json> --answers <a.json> [--laws <path>]` records
//! the expert's answers in `.metaform/laws.tsv` (`yes` as a law, `no` as a refutation with a
//! validated counterexample, `define` as a law) and adopts a defined predicate in the state only for
//! a `define` answer. Replaying the same answers file does not duplicate an entry.

/// Results and protocol go to stdout; commentary goes to stderr through `tracing`.
///
/// The kernel library prints nothing at all: this binary is the only place that owns the two
/// streams, which is what keeps `crates/metaform/src` free of I/O and lets the MCP server promise that every
/// line on stdout is protocol.
macro_rules! out {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

macro_rules! note {
    ($($arg:tt)*) => {{
        tracing::info!($($arg)*);
    }};
}

/// Install the stderr subscriber once, at startup. `METAFFORM_LOG` sets the level (off, error, warn,
/// info, debug, trace); anything unparsable falls back to info rather than failing a run over a
/// logging variable.
fn init_telemetry() {
    use tracing_subscriber::filter::LevelFilter;
    let level = match std::env::var("METAFFORM_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "off" => LevelFilter::OFF,
        "error" => LevelFilter::ERROR,
        "warn" => LevelFilter::WARN,
        "debug" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => LevelFilter::INFO,
    };
    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}

use std::io::{self, BufRead, Write};

mod report;
mod score;
use metaform::{
    answer, apply, bit, flag_value, kv, laws, ledger, load_state, mcp, plan, predictions,
    project_root, same_path, state, Definition, DefOp, Explorer, Predicate, Status, Vocabulary,
};

fn ids_to_mask(ids: &[String], n: usize) -> Result<u64, String> {
    let mut m = 0u64;
    for s in ids {
        let x: usize = s.parse().map_err(|_| format!("not a number: {s}"))?;
        if x >= n {
            return Err(format!("object {x} out of range (object count {n})"));
        }
        m |= bit(x);
    }
    Ok(m)
}

fn pairs_between(e: &Explorer, sp: u64, sn: u64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for c in e.classes() {
        let ins: Vec<usize> = (0..e.n).filter(|&x| (c & bit(x)) != 0 && (sp & bit(x)) != 0).collect();
        let outs: Vec<usize> = (0..e.n).filter(|&x| (c & bit(x)) != 0 && (sn & bit(x)) != 0).collect();
        for a in &ins {
            for b in &outs {
                out.push((*a, *b));
            }
        }
    }
    out
}

fn colliding_pairs(e: &Explorer) -> Vec<(usize, usize)> {
    pairs_between(e, e.pos, e.neg)
}

fn json_ids(v: &[usize]) -> String {
    let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn json_pairs(p: &[(usize, usize)]) -> String {
    let items: Vec<String> = p.iter().map(|(a, b)| format!("[{a},{b}]")).collect();
    format!("[{}]", items.join(","))
}

/// One representative per undecided equivalence class (the suggestion for a batch round of labeling).
fn free_reps(e: &Explorer) -> Vec<usize> {
    let known = e.pos | e.neg;
    e.classes()
        .iter()
        .filter(|c| (**c & known) == 0 && **c != 0)
        .map(|c| c.trailing_zeros() as usize)
        .collect()
}

fn explain(e: &Explorer) {
    out!("--- equivalence classes ({}) ---", e.classes().len());
    for (i, c) in e.classes().iter().enumerate() {
        let members: Vec<String> = (0..e.n)
            .filter(|&x| c & bit(x) != 0)
            .map(|x| {
                let tag = if e.pos & bit(x) != 0 {
                    "+"
                } else if e.neg & bit(x) != 0 {
                    "-"
                } else {
                    " "
                };
                format!("{tag}{x}")
            })
            .collect();
        let mark = if (c & e.pos) != 0 && (c & e.neg) != 0 { "  <== collision" } else { "" };
        out!("  [{i:2}] {}{}", members.join(" "), mark);
    }
    let pairs = colliding_pairs(e);
    if pairs.is_empty() {
        out!("colliding pairs: none");
    } else {
        let shown: Vec<String> = pairs.iter().take(8).map(|(a, b)| format!("({a},{b})")).collect();
        out!("colliding pairs: {}  {}", pairs.len(), shown.join(" "));
        if pairs.len() > 8 {
            out!("  (showing only the first 8 pairs)");
        }
    }
}

fn mask_to_ids(mask: u64, n: usize) -> Vec<usize> {
    (0..n).filter(|&x| mask & bit(x) != 0).collect()
}

/// Metarule-style candidate generation: conjunction/disjunction/negation over existing predicates.
/// Note: all these instances fall inside `Def(V)`, so they do **not** produce a new atom.
fn gen_candidates(preds: &[(String, u64)], n: usize) -> Vec<(String, u64)> {
    use std::collections::HashSet;
    let mut seen: HashSet<u64> = preds.iter().map(|(_, m)| *m).collect();
    let mut out: Vec<(String, u64)> = Vec::new();
    for (i, (ni, mi)) in preds.iter().enumerate() {
        let nm = !mi & ((1u64 << n) - 1);
        if nm != 0 && seen.insert(nm) {
            out.push((format!("not({ni})"), nm));
        }
        for (nj, mj) in preds.iter().skip(i + 1) {
            let a = mi & mj;
            if a != 0 && seen.insert(a) {
                out.push((format!("and({ni},{nj})"), a));
            }
            let o = mi | mj;
            if o != 0 && seen.insert(o) {
                out.push((format!("or({ni},{nj})"), o));
            }
        }
    }
    out
}

fn status(e: &Explorer) {
    let st = match e.status() {
        Status::Insufficient => "insufficient (vocabulary proven insufficient)",
        Status::Undetermined => "undetermined (open classes remain)",
        Status::Identified => "identified (extension fully determined)",
    };
    let free = e.classes().iter().filter(|c| (*c & (e.pos | e.neg)) == 0).count();
    out!("status: {st}");
    out!("open classes: {free}   capacity: 2^{} = {}", e.classes().len(), e.capacity());
}

// ---------------- project directory and ledger subcommands ----------------

/// Parse the `--define` spelling: `all-of:a,b` or `any-of:a,b`.
///
/// The operator uses the same hyphenated style as the CLI's other flags; the protocol keeps its
/// `all_of` / `any_of` names. Underscore spellings are accepted too, so a name copied from a
/// `metaform.propose/1` response is not rejected.
fn parse_define(raw: &str) -> Result<Definition, String> {
    let (op, operand_list) = raw.split_once(':').ok_or_else(|| {
        format!("--define expects `all-of:<name,...>` or `any-of:<name,...>`, got `{raw}`")
    })?;
    let op = match op.trim() {
        "all-of" | "all_of" => DefOp::AllOf,
        "any-of" | "any_of" => DefOp::AnyOf,
        other => {
            return Err(format!(
                "unknown definition operator `{other}` (expected all-of or any-of)"
            ))
        }
    };
    let mut operands: Vec<String> = Vec::new();
    for part in operand_list.split(',') {
        let name = part.trim();
        if name.is_empty() {
            return Err(format!("--define has an empty operand in `{raw}`"));
        }
        if operands.iter().any(|seen| seen == name) {
            return Err(format!("--define repeats operand `{name}`"));
        }
        operands.push(name.to_string());
    }
    // Repeated operands are rejected above rather than silently merged, so canonicalising here only
    // fixes the order: `all-of:b,a` is stored as `all_of:[a,b]`.
    Ok(Definition::new(op, operands))
}

/// Parse a numeric flag, keeping "absent" distinct from "present but malformed".
fn parse_num(args: &[String], flag: &str, default: usize) -> Result<usize, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(default),
        Some(i) => match args.get(i + 1) {
            Some(raw) => raw
                .parse::<usize>()
                .map_err(|_| format!("{flag} expects a non-negative integer, got `{raw}`")),
            None => Err(format!("{flag} expects a value")),
        },
    }
}

/// `metaform suggest --project . [--max-arity 2] [--max 20] [--emit-request [--out <path>]]`.
///
/// Read-only: the state is loaded and printed; it is never saved or extended. With `--emit-request`
/// the enumeration is skipped: an `INSUFFICIENT` state prints a ready-to-fill `metaform.propose/1`
/// request instead (to stdout, or to `--out`), and any other status prints
/// `{"status":"<STATUS>","request":null}`.
fn cmd_suggest(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    let Some(st) = load_state(&root.join("state.json"), "suggest") else {
        return 2;
    };
    let max_arity = match parse_num(args, "--max-arity", 2) {
        Ok(0) => {
            eprintln!("metaform suggest: --max-arity must be at least 1");
            return 2;
        }
        Ok(v) => v,
        Err(e) => {
            eprintln!("metaform suggest: {e}");
            return 2;
        }
    };
    let max = match parse_num(args, "--max", 20) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("metaform suggest: {e}");
            return 2;
        }
    };
    if args.iter().any(|a| a == "--emit-request") {
        let explorer = st.explorer();
        let (status, request) = metaform::suggest::request_for_state(&explorer);
        match request {
            Some(request) => match kv(args, "--out") {
                Some(path) => {
                    if let Err(e) = std::fs::write(&path, format!("{request}\n")) {
                        eprintln!("metaform suggest: cannot write request to `{path}`: {e}");
                        return 2;
                    }
                }
                None => out!("{request}"),
            },
            // Nothing to fill: the state does not yet need a predicate outside the vocabulary.
            None => out!(
                "{{\"status\":\"{}\",\"request\":null}}",
                metaform::suggest::status_str(status)
            ),
        }
        return 0;
    }
    let report = metaform::suggest::suggest(&st.explorer(), max_arity, max);
    out!("{}", metaform::suggest::to_json(&report));
    0
}

/// `metaform explore --project . [--max-questions N] [--laws <path>] [--emit-questions <path>] [--out <path>]`.
///
/// Read-only attribute exploration. The evidence oracle is the set of objects with a confirmed
/// label under the recommended `theta` (`strong`), the same `theta` whose state status is reported
/// in `status`. The state is only read. `--laws` seeds the initial theory (default
/// `.metaform/laws.tsv`, absent = empty), so a premise a law or a refutation already decides is not
/// asked again. `--emit-questions` writes the open questions for `metaform answer` and suppresses
/// the normal stdout line (`--out` still writes it). `--out` writes the report line to a file and
/// touches nothing else.
fn cmd_explore(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    let Some(st) = load_state(&root.join("state.json"), "explore") else {
        return 2;
    };
    let max = match parse_num(args, "--max-questions", metaform::explore::DEFAULT_BUDGET) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("metaform explore: {e}");
            return 2;
        }
    };
    let laws_path = match flag_value(args, "--laws") {
        Ok(Some(path)) => std::path::PathBuf::from(path),
        Ok(None) => root.join("laws.tsv"),
        Err(e) => {
            eprintln!("metaform explore: {e}");
            return 2;
        }
    };
    let emit_path = match flag_value(args, "--emit-questions") {
        Ok(path) => path,
        Err(e) => {
            eprintln!("metaform explore: {e}");
            return 2;
        }
    };
    if st.n > 64 {
        eprintln!("metaform explore: at most 64 objects are supported (state has {})", st.n);
        return 2;
    }
    let e = st.explorer();
    let theta = e.recommended_theta();
    let (sp, sn) = e.strong(theta);
    let context = match metaform::explore::Context::from_explorer(&e, sp | sn) {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("metaform explore: {msg}");
            return 2;
        }
    };
    let loaded = match laws::load(&laws_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("metaform explore: {e}");
            return 2;
        }
    };
    let (theory, recorded) = match laws::masks(&loaded, &context.names) {
        Ok(parts) => parts,
        Err(e) => {
            eprintln!("metaform explore: {e}");
            return 2;
        }
    };
    let refutations: Vec<metaform::explore::Refutation> = recorded
        .into_iter()
        .map(|entry| metaform::explore::Refutation {
            premise: entry.premise,
            conclusion: entry.conclusion,
            counterexample: entry.counterexample,
            source: entry.source,
        })
        .collect();
    let report =
        context.run_with_theory_and_refutations(max, e.status_with(theta), &theory, &refutations);
    let line = metaform::explore::to_json(&report);
    let state_path = root.join("state.json");
    let out_path = kv(args, "--out");

    // Refuse every path collision before writing anything: the state is read-only, and a report or
    // questions file must not silently replace the law theory either.
    for (flag, path) in [
        ("--out", out_path.as_deref()),
        ("--emit-questions", emit_path.as_deref()),
    ] {
        let Some(path) = path else { continue };
        let path = std::path::Path::new(path);
        if same_path(path, &state_path) {
            eprintln!("metaform explore: {flag} must not overwrite the project state");
            return 2;
        }
        if same_path(path, &laws_path) {
            eprintln!("metaform explore: {flag} must not overwrite the law theory");
            return 2;
        }
    }
    if let (Some(out), Some(questions)) = (out_path.as_deref(), emit_path.as_deref()) {
        if same_path(std::path::Path::new(out), std::path::Path::new(questions)) {
            eprintln!("metaform explore: --out and --emit-questions must name different files");
            return 2;
        }
    }

    if let Some(path) = emit_path.as_deref() {
        let text = format!("{}\n", metaform::explore::questions_json(&report));
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("metaform explore: cannot write `{path}`: {e}");
            return 2;
        }
    }
    match out_path {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, format!("{line}\n")) {
                eprintln!("metaform explore: cannot write `{path}`: {e}");
                return 2;
            }
        }
        // With --emit-questions the machine-readable output is the questions file; the report line
        // is only printed when no questions file was requested.
        None if emit_path.is_none() => out!("{line}"),
        None => {}
    }
    0
}

/// `metaform coverage [--project <dir>] [--format json|text] [<table.tsv> | --input <table.tsv>]`.
///
/// Read-only. The table defaults to `<project>/.metaform/coverage.tsv`; a positional path or
/// `--input` names another one. The whole file is parsed and validated before anything is printed,
/// so a malformed table exits 2 with empty stdout and touches no file. `--format text` prints one
/// `item: requirement: reason` line per problem instead of the JSON object.
fn cmd_coverage(args: &[String]) -> i32 {
    let mut project = std::path::PathBuf::from(".");
    let mut format = String::from("json");
    let mut input: Option<std::path::PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        let flag = &args[i];
        match flag.as_str() {
            "--project" | "--format" | "--input" => {
                let Some(value) = args.get(i + 1) else {
                    eprintln!("metaform coverage: {flag} expects a value");
                    return 2;
                };
                if value.starts_with("--") {
                    eprintln!("metaform coverage: {flag} expects a value");
                    return 2;
                }
                match flag.as_str() {
                    "--project" => project = std::path::PathBuf::from(value),
                    "--format" => format = value.clone(),
                    _ => {
                        if input.is_some() {
                            eprintln!("metaform coverage: give the table once (positional path or --input)");
                            return 2;
                        }
                        input = Some(std::path::PathBuf::from(value));
                    }
                }
                i += 2;
            }
            other if other.starts_with("--") => {
                eprintln!("metaform coverage: unknown option `{other}`");
                return 2;
            }
            other => {
                if input.is_some() {
                    eprintln!("metaform coverage: give the table once (positional path or --input)");
                    return 2;
                }
                input = Some(std::path::PathBuf::from(other));
                i += 1;
            }
        }
    }
    if format != "json" && format != "text" {
        eprintln!("metaform coverage: --format expects `json` or `text`, got `{format}`");
        return 2;
    }
    let path = input.unwrap_or_else(|| project.join(".metaform").join("coverage.tsv"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform coverage: cannot read `{}`: {e}", path.display());
            return 2;
        }
    };
    let table = match metaform::coverage::parse(&text) {
        Ok(table) => table,
        Err(e) => {
            eprintln!("metaform coverage: {e}");
            return 2;
        }
    };
    let report = metaform::coverage::analyze(&table);
    match format.as_str() {
        "text" => print!("{}", metaform::coverage::to_text(&report)),
        _ => out!("{}", metaform::coverage::to_json(&report)),
    }
    0
}

fn cmd_init(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    for sub in ["logs", "out", "cache"] {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
    let cfg = root.join("config.toml");
    if !cfg.exists() {
        let _ = std::fs::write(&cfg, "# metaform project config\ntheta = 3\nversion = 1\n");
    }
    let led = root.join("ledger.tsv");
    if !led.exists() {
        let _ = std::fs::write(&led, ledger::HEADER);
    }
    let shown = root.display().to_string().replace('\\', "/");
    out!("{{\"ok\":true,\"dir\":\"{shown}\"}}");
    0
}

fn cmd_ledger(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    let _ = std::fs::create_dir_all(&root);
    let path = root.join("ledger.tsv");
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "add" => {
            let case: usize = kv(args, "--case").and_then(|s| s.parse().ok()).unwrap_or(0);
            let by = kv(args, "--by").unwrap_or_else(|| "manual".into());
            let round: u32 = kv(args, "--round").and_then(|s| s.parse().ok()).unwrap_or(0);
            let rel: f64 = kv(args, "--reliability").and_then(|s| s.parse().ok()).unwrap_or(1.0);
            let answer = kv(args, "--answer").map(|s| s != "false").unwrap_or(true);
            let reason = kv(args, "--reason").unwrap_or_default();
            let all = ledger::read_all(&path);
            let id = ledger::next_id(&all);
            let e = ledger::Entry {
                id: id.clone(),
                case,
                status: "active".into(),
                by,
                round,
                reliability: rel,
                answer,
                reason,
            };
            ledger::append(&path, &e);
            out!("{{\"ok\":true,\"id\":\"{id}\",\"case\":{case}}}");
            0
        }
        "retract" => {
            let id = kv(args, "--id").unwrap_or_default();
            let reason = kv(args, "--reason").unwrap_or_default();
            let all = ledger::read_all(&path);
            match all.iter().rev().find(|e| e.id == id) {
                Some(prev) => {
                    let mut e = prev.clone();
                    e.status = "retracted".into();
                    e.reason = reason;
                    ledger::append(&path, &e);
                    out!("{{\"ok\":true,\"id\":\"{id}\",\"status\":\"retracted\"}}");
                    0
                }
                None => {
                    eprintln!("no such id: {id}");
                    2
                }
            }
        }
        "list" => {
            let show_all = args.iter().any(|a| a == "--all");
            for e in ledger::fold(&ledger::read_all(&path)) {
                if show_all || e.status == "active" {
                    out!(
                        "{}\t{}\t{}\t{}\t{}\t{:.2}\t{}\t{}",
                        e.id, e.case, e.status, e.by, e.round, e.reliability, e.answer, e.reason
                    );
                }
            }
            0
        }
        "conflicts" => {
            let threshold: f64 = kv(args, "--threshold").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let ratio: f64 = kv(args, "--ratio").and_then(|s| s.parse().ok()).unwrap_or(0.7);
            let conf = ledger::conflicts(&ledger::read_all(&path), threshold, ratio);
            for (case, s) in &conf {
                out!("CONFLICT case {case}: pos={:.2} neg={:.2}", s.pos, s.neg);
            }
            let items: Vec<String> = conf.keys().map(|c| c.to_string()).collect();
            out!("{{\"conflicts\":[{}]}}", items.join(","));
            0
        }
        "cases" => {
            let threshold: f64 = kv(args, "--threshold").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let ratio: f64 = kv(args, "--ratio").and_then(|s| s.parse().ok()).unwrap_or(0.7);
            let v = ledger::confirmed_cases(&ledger::read_all(&path), threshold, ratio);
            let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
            out!("{{\"cases\":[{}]}}", items.join(","));
            0
        }
        other => {
            eprintln!("unknown ledger subcommand: {other}");
            2
        }
    }
}

fn cmd_state(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    let path = root.join("state.json");
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("show");
    match sub {
        "init" => {
            let n: usize = kv(args, "--n").and_then(|s| s.parse().ok()).unwrap_or(0);
            let st = state::State { n, ..Default::default() };
            let _ = st.save(&path);
            out!("{{\"ok\":true,\"n\":{n},\"file\":\"{}\"}}", path.display().to_string().replace('\\', "/"));
            0
        }
        "add" => {
            let Some(mut st) = load_state(&path, "state add") else {
                return 2;
            };
            let name = kv(args, "--name").unwrap_or_else(|| "p".into());
            let define = match kv(args, "--define") {
                Some(raw) => match parse_define(&raw) {
                    Ok(def) => Some(def),
                    Err(e) => {
                        eprintln!("metaform state add: {e}");
                        return 2;
                    }
                },
                None => None,
            };
            if define.is_some() && args.iter().any(|a| a == "--ids") {
                eprintln!("metaform state add: use either --ids or --define, not both");
                return 2;
            }
            if let Some(def) = define {
                st.predicates.push(state::PredSpec {
                    name: name.clone(),
                    ids: Vec::new(),
                    definition: Some(def),
                    falsifier: None,
                });
                if let Err(e) = st.resolve() {
                    eprintln!("metaform state add: {e}");
                    return 2;
                }
                let ids = st.predicates.last().map(|p| p.ids.clone()).unwrap_or_default();
                let _ = st.save(&path);
                out!("{{\"ok\":true,\"name\":\"{name}\",\"ids\":{}}}", state::ids_json(&ids));
            } else {
                let ids: Vec<usize> = kv(args, "--ids")
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                st.predicates.push(state::PredSpec {
                    name: name.clone(),
                    ids: ids.clone(),
                    definition: None,
                    falsifier: None,
                });
                let _ = st.save(&path);
                out!("{{\"ok\":true,\"name\":\"{name}\",\"ids\":{}}}", state::ids_json(&ids));
            }
            0
        }
        "vote" => {
            let Some(mut st) = load_state(&path, "state vote") else {
                return 2;
            };
            let id: usize = kv(args, "--id").and_then(|s| s.parse().ok()).unwrap_or(0);
            let yes: u8 = kv(args, "--yes").and_then(|s| s.parse().ok()).unwrap_or(0);
            let no: u8 = kv(args, "--no").and_then(|s| s.parse().ok()).unwrap_or(0);
            st.votes.push(state::VoteSpec { id, yes, no });
            let _ = st.save(&path);
            if args.iter().any(|a| a == "--record") {
                let lpath = root.join("ledger.tsv");
                let all = ledger::read_all(&lpath);
                let lid = ledger::next_id(&all);
                let e = ledger::Entry {
                    id: lid.clone(),
                    case: id,
                    status: "active".into(),
                    by: kv(args, "--by").unwrap_or_else(|| "state-vote".into()),
                    round: 0,
                    reliability: 1.0,
                    answer: yes >= no,
                    reason: kv(args, "--reason").unwrap_or_default(),
                };
                ledger::append(&lpath, &e);
                out!("{{\"ok\":true,\"id\":{id},\"yes\":{yes},\"no\":{no},\"ledger\":\"{lid}\"}}");
            } else {
                out!("{{\"ok\":true,\"id\":{id},\"yes\":{yes},\"no\":{no}}}");
            }
            0
        }
        "need" => {
            let Some(st) = load_state(&path, "state need") else {
                return 2;
            };
            let theta: Option<u8> = kv(args, "--theta").and_then(|s| s.parse().ok());
            out!("{}", state::need_json(&st.explorer(), theta));
            0
        }
        "show" => {
            let Some(st) = load_state(&path, "state show") else {
                return 2;
            };
            out!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
            0
        }
        other => {
            eprintln!("unknown state subcommand: {other}");
            2
        }
    }
}

fn cmd_check_predictions(args: &[String]) -> i32 {
    // This wrapper lives here rather than in the crate that owns the file: `metaform-store` reads and
    // writes the four artefacts under `.metaform/` and does not parse a command line. A persistence
    // crate with a `--project` flag would be a store that knows how it is invoked.
    match predictions::check(&project_root(args)) {
        Ok(line) => {
            out!("{line}");
            0
        }
        Err(e) => {
            eprintln!("metaform check-predictions: {e}");
            2
        }
    }
}

/// `metaform propose --request <path> --response <path>`
///
/// Exit 2 only when the request itself cannot be used. A malformed, mistargeted or refused response
/// is a successful validation that prints its verdict, because the verdict is the product here.
///
/// The wrapper lives in the binary, not in `metaform-propose`: the crate is a pure function from two
/// byte strings to one, and a crate that also parsed `--request` would need to know how it is called.
fn cmd_propose(args: &[String]) -> i32 {
    let (request_path, response_path) = match (
        flag_value(args, "--request"),
        flag_value(args, "--response"),
    ) {
        (Ok(Some(request)), Ok(Some(response))) => (request, response),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("metaform propose: {e}");
            return 2;
        }
        _ => {
            eprintln!("usage: metaform propose --request <request.json> --response <response.json>");
            return 2;
        }
    };
    let request_text = match std::fs::read_to_string(&request_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform propose: cannot read request file `{request_path}`: {e}");
            return 2;
        }
    };
    // A missing or unreadable response is a protocol failure, not a CLI failure: the empty string is
    // a response the validator can reject, and its rejection is a verdict worth printing.
    let response_text = std::fs::read_to_string(&response_path).unwrap_or_default();

    match metaform::propose::validate_response(&request_text, &response_text) {
        Ok(verdict) => {
            out!("{}", metaform::propose::verdict_json(&verdict));
            0
        }
        Err(e) => {
            eprintln!("metaform propose: invalid request in `{request_path}`: {e}");
            2
        }
    }
}

fn cmd_plan(args: &[String]) -> i32 {
    let root = project_root(args).join(".metaform");
    let Some(st) = load_state(&root.join("state.json"), "plan") else {
        return 2;
    };
    let rows = ledger::read_all(&root.join("ledger.tsv"));
    let e = st.explorer();
    let theta: u8 = kv(args, "--theta")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| e.recommended_theta());
    let pool: Option<Vec<(String, u64)>> = kv(args, "--pool")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| plan::parse_pool(&text));
    let p = plan::build(&e, &rows, theta, pool.as_deref());
    out!("{}", plan::to_json(&p));
    0
}

fn cmd_report(args: &[String]) -> i32 {
    let root = project_root(args);
    let md = match report::render(&root) {
        Ok(md) => md,
        Err(e) => {
            eprintln!("metaform report: {e}");
            return 2;
        }
    };
    let out = root.join(".metaform").join("out");
    let _ = std::fs::create_dir_all(&out);
    let path = out.join("report.md");
    match std::fs::write(&path, md) {
        Ok(_) => {
            out!("{}", path.display().to_string().replace('\\', "/"));
            0
        }
        Err(e) => {
            eprintln!("cannot write the report: {e}");
            1
        }
    }
}

fn cmd_score(args: &[String]) -> i32 {
    let root = project_root(args);
    let dir = root.join(".metaform");
    let Some(st) = load_state(&dir.join("state.json"), "score") else {
        return 2;
    };
    let rows_ledger = ledger::read_all(&dir.join("ledger.tsv"));
    let e = st.explorer();
    let theta = e.recommended_theta();
    let pairs = plan::colliding(&e, theta);

    let Some(pool_path) = kv(args, "--pool") else {
        eprintln!("score requires --pool (candidate extensions)");
        return 2;
    };
    let pool_text = std::fs::read_to_string(&pool_path).unwrap_or_default();
    let cands = score::parse_candidates(&pool_text);

    let truth = kv(args, "--truth")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| score::parse_truth(&t))
        .unwrap_or_default();
    let confirmed: std::collections::BTreeSet<usize> =
        ledger::confirmed_cases(&rows_ledger, 0.5, 0.7).into_iter().collect();

    let rows = score::score(&cands, &truth, &pairs, &confirmed);
    out!("{}", score::to_json(&rows));
    0
}

fn main() {
    init_telemetry();
    match std::env::args().nth(1).as_deref() {
        // Answered here rather than falling through to the script runner, which would try to open
        // `--version` as a file and panic. An installed binary is checked with these two flags.
        Some("-V" | "--version") => {
            out!("metaform {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("-h" | "--help") => {
            out!("metaform {}", env!("CARGO_PKG_VERSION"));
            out!("usage: metaform <command> [options]  (no argument: interactive script)");
            out!("commands: init, state, ledger, concept, axes, plan, report, score, ops, portfolio, mcp,");
            out!("          suggest, explore, coverage, answer, apply, check-predictions, propose,");
            out!("          translate, check");
            out!("MCP: metaform mcp  (newline-delimited JSON-RPC over stdio)");
            return;
        }
        Some("init") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_init(&v));
        }
        Some("ledger") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_ledger(&v));
        }
        Some("concept") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(metaform::concept::cmd(&v));
        }
        Some("axes") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(metaform::axes::cmd(&v));
        }
        Some("state") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_state(&v));
        }
        Some("suggest") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_suggest(&v));
        }
        Some("explore") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_explore(&v));
        }
        Some("coverage") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_coverage(&v));
        }
        Some("answer") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(answer::cmd(&v));
        }
        Some("score") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_score(&v));
        }
        Some("report") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_report(&v));
        }
        Some("plan") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_plan(&v));
        }
        Some("propose") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_propose(&v));
        }
        Some("apply") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(apply::cmd(&v));
        }
        Some("check-predictions") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_check_predictions(&v));
        }
        Some("translate") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            match metaform::morphism::run(&v) {
                Ok(line) => {
                    out!("{line}");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("metaform translate: {e}");
                    std::process::exit(2);
                }
            }
        }
        Some("ops") => {
            out!("{}", metaform::operators::demo_json());
            std::process::exit(0);
        }
        Some("portfolio") => {
            let v: Vec<String> = std::env::args().skip(2).collect();
            std::process::exit(metaform::portfolio::run_cli(&v));
        }
        Some("check") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(metaform::check::cmd(&v));
        }
        Some("metrics") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(metaform::metrics::cmd(&v));
        }
        Some("mcp") => {
            mcp::serve();
            std::process::exit(0);
        }
        _ => {}
    }    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut e: Option<Explorer> = None;

    let run = |line: &str, e: &mut Option<Explorer>| -> bool {
        let line = line.split('#').next().unwrap_or("").trim().to_string();
        if line.is_empty() {
            return true;
        }
        let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
        let cmd = parts[0].as_str();
        let rest = &parts[1..];
        match cmd {
            "objects" => {
                let n: usize = rest[0].parse().expect("objects needs a number");
                *e = Some(Explorer::new(n, Vocabulary::default()));
                out!("scenario created: {n} objects");
            }
            "add" | "score" => {
                let ex = e.as_mut().expect("run objects first");
                let name = rest[0].clone();
                let mask = ids_to_mask(&rest[1..], ex.n).expect("invalid object id");
                if cmd == "score" {
                    let pairs = colliding_pairs(ex);
                    let split = pairs.iter().filter(|(a, b)| ((mask >> a) & 1) != ((mask >> b) & 1)).count();
                    out!("score {name}: splits {split}/{} colliding pairs", pairs.len());
                } else {
                    ex.propose(Predicate::new(name, mask));
                    out!("predicate added (vocabulary has {} entries)", ex.vocab.len());
                }
            }
            "pos" | "neg" => {
                let ex = e.as_mut().expect("run objects first");
                let mask = ids_to_mask(rest, ex.n).expect("invalid object id");
                let member = cmd == "pos";
                for x in 0..ex.n {
                    if mask & bit(x) != 0 {
                        ex.answer(x, member);
                    }
                }
                out!("labeling done: pos {} / neg {}", ex.pos.count_ones(), ex.neg.count_ones());
            }
            "ask" => {
                let ex = e.as_ref().expect("run objects first");
                let theta: u8 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(3);
                match ex.next_question_with(theta) {
                    Some(x) => out!("suggested question: object {x} (theta={theta}; after answering, write pos/vote)"),
                    None => out!("no object left to ask (status: {:?})", ex.status_with(theta)),
                }
            }
            "status" => status(e.as_ref().expect("run objects first")),
            "vote" => {
                // answer with vote counts: vote <id> <yes> <no>
                let ex = e.as_mut().expect("run objects first");
                let x: usize = rest.first().and_then(|s| s.parse().ok()).expect("vote needs an object id");
                let yes: u8 = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                let no: u8 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                ex.answer_votes(x, yes, no);
                out!("vote recorded: obj {x} = ({yes}, {no})");
            }
            "need" => {
                let ex = e.as_ref().expect("run objects first");
                let theta: u8 = rest
                    .first()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| ex.recommended_theta());
                let (sp, sn) = ex.strong(theta);
                let pairs = pairs_between(ex, sp, sn);
                let free = free_reps(ex);
                let unverified = ex.unprobed();
                let (st, action) = match ex.status_with(theta) {
                    Status::Insufficient => ("INSUFFICIENT", "PROPOSE"),
                    Status::Undetermined => ("UNDETERMINED", "ASK"),
                    Status::Identified if !unverified.is_empty() => ("IDENTIFIED", "VERIFY"),
                    Status::Identified => ("IDENTIFIED", "STOP"),
                };
                let pending = ex.pending(theta);
                let eps_hat = ex.implied_epsilon();
                let theta_rec = ex.recommended_theta();
                out!(
                    "{{\"status\":\"{st}\",\"action\":\"{action}\",\"theta\":{theta},\"epsilon_hat\":{eps_hat:.2},\"theta_rec\":{theta_rec},\"classes\":{},\"free_classes\":{},\"colliding_pairs\":{},\"pending\":{},\"unverified\":{},\"suggested_labels\":{}}}",
                    ex.classes().len(),
                    free.len(),
                    json_pairs(&pairs),
                    json_ids(&pending),
                    json_ids(&unverified),
                    json_ids(&free)
                );
            }
            "gen" => {
                // generate a candidate predicate pool from the current vocabulary (metarule-style combinations). Can write a file for cover to use.
                let ex = e.as_ref().expect("run objects first");
                let preds: Vec<(String, u64)> = ex
                    .vocab
                    .preds
                    .iter()
                    .map(|p| (p.name.clone(), p.ext))
                    .collect();
                let cands = gen_candidates(&preds, ex.n);
                let mut text = String::new();
                for (name, m) in &cands {
                    let ids: Vec<String> =
                        mask_to_ids(*m, ex.n).iter().map(|x| x.to_string()).collect();
                    text.push_str(&format!("{name} {}\n", ids.join(" ")));
                }
                match rest.first() {
                    Some(path) => {
                        std::fs::write(path, &text).expect("cannot write the pool file");
                        out!("generated {} candidates → {path}", cands.len());
                    }
                    None => print!("{text}"),
                }
            }
            "cover" => {
                // read a candidate predicate pool and greedily cover all colliding pairs (the hands-on version of T7).
                let ex = e.as_ref().expect("run objects first");
                let path = rest.first().expect("cover needs a pool file path");
                let text = std::fs::read_to_string(path).expect("cannot read the pool file");
                let mut pool: Vec<(String, u64)> = Vec::new();
                for line in text.lines() {
                    let line = line.split('#').next().unwrap_or("").trim();
                    if line.is_empty() {
                        continue;
                    }
                    let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
                    let name = parts[0].clone();
                    let mask = match ids_to_mask(&parts[1..], ex.n) {
                        Ok(m) => m,
                        Err(e) => {
                            out!("pool file error: {e}");
                            return true;
                        }
                    };
                    pool.push((name, mask));
                }
                let mut pairs = colliding_pairs(ex);
                let total = pairs.len();
                let mut chosen: Vec<String> = Vec::new();
                while !pairs.is_empty() && !pool.is_empty() {
                    let mut best: Option<(usize, usize)> = None;
                    for (i, (_, m)) in pool.iter().enumerate() {
                        let c = pairs
                            .iter()
                            .filter(|(a, b)| ((m >> a) & 1) != ((m >> b) & 1))
                            .count();
                        if best.map_or(true, |(_, bc)| c > bc) {
                            best = Some((i, c));
                        }
                    }
                    let (idx, covered) = best.expect("pool non-empty");
                    if covered == 0 {
                        break;
                    }
                    let (name, mask) = pool.remove(idx);
                    out!("cover: {name} splits {covered}/{} pairs", pairs.len());
                    pairs.retain(|(a, b)| ((mask >> a) & 1) == ((mask >> b) & 1));
                    chosen.push(name);
                }
                let chosen_json = format!(
                    "[{}]",
                    chosen.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",")
                );
                out!(
                    "{{\"chosen\":{chosen_json},\"covered\":{},\"total\":{},\"remaining\":{}}}",
                    total - pairs.len(),
                    total,
                    pairs.len()
                );
            }
            "batch" => {
                let ex = e.as_ref().expect("run objects first");
                let k: usize = rest.first().and_then(|s| s.parse().ok()).unwrap_or(4);
                let mut reps = free_reps(ex);
                reps.truncate(k);
                out!(
                    "{{\"round\":{k},\"suggested_labels\":{},\"free_classes\":{}}}",
                    json_ids(&reps),
                    free_reps(ex).len()
                );
            }
            "explain" => explain(e.as_ref().expect("run objects first")),
            "capacity" => {
                let ex = e.as_ref().expect("run objects first");
                out!("2^{} = {}", ex.classes().len(), ex.capacity());
            }
            "quit" => return false,
            other => note!("unknown command: {other}"),
        }
        true
    };

    match args.first().map(|s| s.as_str()) {
        Some(path) => {
            let text = std::fs::read_to_string(path).expect("cannot read the script");
            for line in text.lines() {
                if !run(line, &mut e) {
                    break;
                }
            }
        }
        None => {
            note!("metaform: type a command and press Enter (quit to exit)");
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line.unwrap_or_default();
                if !run(&line, &mut e) {
                    break;
                }
                let _ = io::stdout().flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `propose` reads two files and prints one line. It must not create `.metaform/`, a ledger, or
    /// any other side file: a command that owns no state cannot corrupt the state it was asked to
    /// inspect, and this is the property that makes it safe to run against a live project.
    #[test]
    fn propose_writes_nothing_but_its_verdict() {
        let dir = std::env::temp_dir().join(format!("metaform-propose-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let request = dir.join("request.json");
        let response = dir.join("response.json");
        std::fs::write(
            &request,
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","objects":3,"vocabulary":[],"evidence":{"pos":[0],"neg":[1]},"limit":8}"#,
        )
        .expect("write request");
        std::fs::write(
            &response,
            r#"{"protocol":"metaform.propose/1","request_id":"r-1","candidates":[{"name":"only0","ids":[0]}]}"#,
        )
        .expect("write response");

        let args: Vec<String> = [
            "propose",
            "--request",
            request.to_str().expect("utf-8 path"),
            "--response",
            response.to_str().expect("utf-8 path"),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(cmd_propose(&args), 0, "a usable request and response exit 0");
        assert_eq!(cmd_propose(&args), 0, "and they do it twice");

        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("dir entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["request.json".to_string(), "response.json".to_string()],
            "propose must not write any file"
        );
        assert!(!dir.join(".metaform").exists(), "propose must not create a state directory");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
