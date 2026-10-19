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

use std::io::{self, BufRead, Write};

mod ledger;
mod mcp;
mod plan;
mod report;
mod score;
mod state;
use metaform::{bit, Definition, DefOp, Explorer, Predicate, Status, Vocabulary};

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
    println!("--- equivalence classes ({}) ---", e.classes().len());
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
        println!("  [{i:2}] {}{}", members.join(" "), mark);
    }
    let pairs = colliding_pairs(e);
    if pairs.is_empty() {
        println!("colliding pairs: none");
    } else {
        let shown: Vec<String> = pairs.iter().take(8).map(|(a, b)| format!("({a},{b})")).collect();
        println!("colliding pairs: {}  {}", pairs.len(), shown.join(" "));
        if pairs.len() > 8 {
            println!("  (showing only the first 8 pairs)");
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
    println!("status: {st}");
    println!("open classes: {free}   capacity: 2^{} = {}", e.classes().len(), e.capacity());
}

// ---------------- project directory and ledger subcommands ----------------

fn kv(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn project_root(args: &[String]) -> std::path::PathBuf {
    std::path::PathBuf::from(kv(args, "--project").unwrap_or_else(|| ".".into()))
}

/// Load the project state, reporting an unresolvable definition (missing operand, cycle, bad file)
/// on stderr and returning `None` so the caller can exit with status 2.
fn load_state(path: &std::path::Path, command: &str) -> Option<state::State> {
    match state::State::load(path) {
        Ok(st) => Some(st),
        Err(e) => {
            eprintln!("metaform {command}: {e}");
            None
        }
    }
}

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
    Ok(Definition { op, operands })
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
    println!("{{\"ok\":true,\"dir\":\"{shown}\"}}");
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
            println!("{{\"ok\":true,\"id\":\"{id}\",\"case\":{case}}}");
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
                    println!("{{\"ok\":true,\"id\":\"{id}\",\"status\":\"retracted\"}}");
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
                    println!(
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
                println!("CONFLICT case {case}: pos={:.2} neg={:.2}", s.pos, s.neg);
            }
            let items: Vec<String> = conf.keys().map(|c| c.to_string()).collect();
            println!("{{\"conflicts\":[{}]}}", items.join(","));
            0
        }
        "cases" => {
            let threshold: f64 = kv(args, "--threshold").and_then(|s| s.parse().ok()).unwrap_or(0.5);
            let ratio: f64 = kv(args, "--ratio").and_then(|s| s.parse().ok()).unwrap_or(0.7);
            let v = ledger::confirmed_cases(&ledger::read_all(&path), threshold, ratio);
            let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
            println!("{{\"cases\":[{}]}}", items.join(","));
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
            println!("{{\"ok\":true,\"n\":{n},\"file\":\"{}\"}}", path.display().to_string().replace('\\', "/"));
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
                });
                if let Err(e) = st.resolve() {
                    eprintln!("metaform state add: {e}");
                    return 2;
                }
                let ids = st.predicates.last().map(|p| p.ids.clone()).unwrap_or_default();
                let _ = st.save(&path);
                println!("{{\"ok\":true,\"name\":\"{name}\",\"ids\":{}}}", state::ids_json(&ids));
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
                });
                let _ = st.save(&path);
                println!("{{\"ok\":true,\"name\":\"{name}\",\"ids\":{}}}", state::ids_json(&ids));
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
                println!("{{\"ok\":true,\"id\":{id},\"yes\":{yes},\"no\":{no},\"ledger\":\"{lid}\"}}");
            } else {
                println!("{{\"ok\":true,\"id\":{id},\"yes\":{yes},\"no\":{no}}}");
            }
            0
        }
        "need" => {
            let Some(st) = load_state(&path, "state need") else {
                return 2;
            };
            let theta: Option<u8> = kv(args, "--theta").and_then(|s| s.parse().ok());
            println!("{}", state::need_json(&st.explorer(), theta));
            0
        }
        "show" => {
            let Some(st) = load_state(&path, "state show") else {
                return 2;
            };
            println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
            0
        }
        other => {
            eprintln!("unknown state subcommand: {other}");
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
    println!("{}", plan::to_json(&p));
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
            println!("{}", path.display().to_string().replace('\\', "/"));
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
    println!("{}", score::to_json(&rows));
    0
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("init") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_init(&v));
        }
        Some("ledger") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_ledger(&v));
        }
        Some("state") => {
            let v: Vec<String> = std::env::args().skip(1).collect();
            std::process::exit(cmd_state(&v));
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
            std::process::exit(metaform::propose::cmd(&v));
        }
        Some("ops") => {
            println!("{}", metaform::operators::demo_json());
            std::process::exit(0);
        }
        Some("portfolio") => {
            let v: Vec<String> = std::env::args().skip(2).collect();
            std::process::exit(metaform::portfolio::run_cli(&v));
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
                println!("scenario created: {n} objects");
            }
            "add" | "score" => {
                let ex = e.as_mut().expect("run objects first");
                let name = rest[0].clone();
                let mask = ids_to_mask(&rest[1..], ex.n).expect("invalid object id");
                if cmd == "score" {
                    let pairs = colliding_pairs(ex);
                    let split = pairs.iter().filter(|(a, b)| ((mask >> a) & 1) != ((mask >> b) & 1)).count();
                    println!("score {name}: splits {split}/{} colliding pairs", pairs.len());
                } else {
                    ex.propose(Predicate::new(name, mask));
                    println!("predicate added (vocabulary has {} entries)", ex.vocab.len());
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
                println!("labeling done: pos {} / neg {}", ex.pos.count_ones(), ex.neg.count_ones());
            }
            "ask" => {
                let ex = e.as_ref().expect("run objects first");
                let theta: u8 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(3);
                match ex.next_question_with(theta) {
                    Some(x) => println!("suggested question: object {x} (theta={theta}; after answering, write pos/vote)"),
                    None => println!("no object left to ask (status: {:?})", ex.status_with(theta)),
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
                println!("vote recorded: obj {x} = ({yes}, {no})");
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
                println!(
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
                        println!("generated {} candidates → {path}", cands.len());
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
                            println!("pool file error: {e}");
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
                    println!("cover: {name} splits {covered}/{} pairs", pairs.len());
                    pairs.retain(|(a, b)| ((mask >> a) & 1) == ((mask >> b) & 1));
                    chosen.push(name);
                }
                let chosen_json = format!(
                    "[{}]",
                    chosen.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(",")
                );
                println!(
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
                println!(
                    "{{\"round\":{k},\"suggested_labels\":{},\"free_classes\":{}}}",
                    json_ids(&reps),
                    free_reps(ex).len()
                );
            }
            "explain" => explain(e.as_ref().expect("run objects first")),
            "capacity" => {
                let ex = e.as_ref().expect("run objects first");
                println!("2^{} = {}", ex.classes().len(), ex.capacity());
            }
            "quit" => return false,
            other => println!("unknown command: {other}"),
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
            println!("metaform: type a command and press Enter (quit to exit)");
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














