//! `metaform` —— 查询驱动表征变更的命令行工作台。
//!
//! 从一个脚本文件（或 stdin）读命令，逐步构造并观察状态机。
//! 它的用途不是跑基准，而是**在思考时把当前场景摆出来**：
//! 现在缺什么、撞车的是哪一对、某个候选谓词能劈开几对。
//!
//! 命令：
//! ```text
//! objects 21                     # 新建场景（对象 0..20）
//! add <name> <id...>             # 向词汇添加谓词，取值为 id 列表
//! pos <id...>   neg <id...>      # 标注正例 / 反例
//! ask                            # 下一个建议提问的对象
//! status                         # 当前状态 + 未决类数
//! explain                        # 打印全部等价类，标出撞车对
//! score <name> <id...>           # 不添加，只报它能劈开几对撞车
//! capacity                       # 2^(类数)
//! ```
//! 行内 `#` 之后是注释。

use std::io::{self, BufRead, Write};

mod ledger;
mod mcp;
mod plan;
mod report;
mod score;
mod state;
use metaform::{bit, Explorer, Predicate, Status, Vocabulary};

fn ids_to_mask(ids: &[String], n: usize) -> Result<u64, String> {
    let mut m = 0u64;
    for s in ids {
        let x: usize = s.parse().map_err(|_| format!("不是数字: {s}"))?;
        if x >= n {
            return Err(format!("对象 {x} 越界（对象数 {n}）"));
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

/// 每个未决等价类取一个代表（一轮批量标注的建议）。
fn free_reps(e: &Explorer) -> Vec<usize> {
    let known = e.pos | e.neg;
    e.classes()
        .iter()
        .filter(|c| (**c & known) == 0 && **c != 0)
        .map(|c| c.trailing_zeros() as usize)
        .collect()
}

fn explain(e: &Explorer) {
    println!("--- 等价类（{} 个）---", e.classes().len());
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
        let mark = if (c & e.pos) != 0 && (c & e.neg) != 0 { "  <== 撞车" } else { "" };
        println!("  [{i:2}] {}{}", members.join(" "), mark);
    }
    let pairs = colliding_pairs(e);
    if pairs.is_empty() {
        println!("撞车对：无");
    } else {
        let shown: Vec<String> = pairs.iter().take(8).map(|(a, b)| format!("({a},{b})")).collect();
        println!("撞车对：{} 个  {}", pairs.len(), shown.join(" "));
        if pairs.len() > 8 {
            println!("  （只显示前 8 对）");
        }
    }
}

fn mask_to_ids(mask: u64, n: usize) -> Vec<usize> {
    (0..n).filter(|&x| mask & bit(x) != 0).collect()
}

/// metarule 式候选生成：对已有谓词做合取/析取/否定。
/// 注意：这些实例都落在 `Def(V)` 内部，**不会**产生新原子。
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
        Status::Insufficient => "insufficient（词汇已被证明不够）",
        Status::Undetermined => "undetermined（还有未决类）",
        Status::Identified => "identified（外延完全确定）",
    };
    let free = e.classes().iter().filter(|c| (*c & (e.pos | e.neg)) == 0).count();
    println!("status: {st}");
    println!("未决类: {free}   容量: 2^{} = {}", e.classes().len(), e.capacity());
}

// ---------------- 项目目录与台账子命令 ----------------

fn kv(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn project_root(args: &[String]) -> std::path::PathBuf {
    std::path::PathBuf::from(kv(args, "--project").unwrap_or_else(|| ".".into()))
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
            let mut st = state::State::load(&path);
            let name = kv(args, "--name").unwrap_or_else(|| "p".into());
            let ids: Vec<usize> = kv(args, "--ids")
                .unwrap_or_default()
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            st.predicates.push(state::PredSpec { name: name.clone(), ids: ids.clone() });
            let _ = st.save(&path);
            println!("{{\"ok\":true,\"name\":\"{name}\",\"ids\":{}}}", state::ids_json(&ids));
            0
        }
        "vote" => {
            let mut st = state::State::load(&path);
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
            let st = state::State::load(&path);
            let theta: Option<u8> = kv(args, "--theta").and_then(|s| s.parse().ok());
            println!("{}", state::need_json(&st.explorer(), theta));
            0
        }
        "show" => {
            let st = state::State::load(&path);
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
    let st = state::State::load(&root.join("state.json"));
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
    let md = report::render(&root);
    let out = root.join(".metaform").join("out");
    let _ = std::fs::create_dir_all(&out);
    let path = out.join("report.md");
    match std::fs::write(&path, md) {
        Ok(_) => {
            println!("{}", path.display().to_string().replace('\\', "/"));
            0
        }
        Err(e) => {
            eprintln!("写不了报告：{e}");
            1
        }
    }
}

fn cmd_score(args: &[String]) -> i32 {
    let root = project_root(args);
    let dir = root.join(".metaform");
    let st = state::State::load(&dir.join("state.json"));
    let rows_ledger = ledger::read_all(&dir.join("ledger.tsv"));
    let e = st.explorer();
    let theta = e.recommended_theta();
    let pairs = plan::colliding(&e, theta);

    let Some(pool_path) = kv(args, "--pool") else {
        eprintln!("score 需要 --pool（候选外延）");
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
        Some("ops") => {
            println!("{}", metaform::operators::demo_json());
            std::process::exit(0);
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
                let n: usize = rest[0].parse().expect("objects 需要数字");
                *e = Some(Explorer::new(n, Vocabulary::default()));
                println!("场景已建立：{n} 个对象");
            }
            "add" | "score" => {
                let ex = e.as_mut().expect("先 objects");
                let name = rest[0].clone();
                let mask = ids_to_mask(&rest[1..], ex.n).expect("对象编号有误");
                if cmd == "score" {
                    let pairs = colliding_pairs(ex);
                    let split = pairs.iter().filter(|(a, b)| ((mask >> a) & 1) != ((mask >> b) & 1)).count();
                    println!("score {name}: 劈开 {split}/{} 对撞车", pairs.len());
                } else {
                    ex.propose(Predicate::new(name, mask));
                    println!("已添加谓词（词汇 {} 个）", ex.vocab.len());
                }
            }
            "pos" | "neg" => {
                let ex = e.as_mut().expect("先 objects");
                let mask = ids_to_mask(rest, ex.n).expect("对象编号有误");
                let member = cmd == "pos";
                for x in 0..ex.n {
                    if mask & bit(x) != 0 {
                        ex.answer(x, member);
                    }
                }
                println!("标注完成：pos {} / neg {}", ex.pos.count_ones(), ex.neg.count_ones());
            }
            "ask" => {
                let ex = e.as_ref().expect("先 objects");
                let theta: u8 = rest.first().and_then(|s| s.parse().ok()).unwrap_or(3);
                match ex.next_question_with(theta) {
                    Some(x) => println!("建议提问：对象 {x}（theta={theta}；回答后写 pos/vote）"),
                    None => println!("没有可问的对象（状态：{:?}）", ex.status_with(theta)),
                }
            }
            "status" => status(e.as_ref().expect("先 objects")),
            "vote" => {
                // 带票数的回答：vote <id> <yes> <no>
                let ex = e.as_mut().expect("先 objects");
                let x: usize = rest.first().and_then(|s| s.parse().ok()).expect("vote 需要对象编号");
                let yes: u8 = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
                let no: u8 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                ex.answer_votes(x, yes, no);
                println!("记录票数：obj {x} = ({yes}, {no})");
            }
            "need" => {
                let ex = e.as_ref().expect("先 objects");
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
                // 从当前词汇生成候选谓词池（metarule 式组合）。可写文件，供 cover 使用。
                let ex = e.as_ref().expect("先 objects");
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
                        std::fs::write(path, &text).expect("写不了池文件");
                        println!("生成 {} 个候选 → {path}", cands.len());
                    }
                    None => print!("{text}"),
                }
            }
            "cover" => {
                // 读一个候选谓词池，贪心覆盖全部撞车对（T7 的动手版本）。
                let ex = e.as_ref().expect("先 objects");
                let path = rest.first().expect("cover 需要池文件路径");
                let text = std::fs::read_to_string(path).expect("读不到池文件");
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
                            println!("池文件错误：{e}");
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
                    println!("cover: {name} 劈开 {covered}/{} 对", pairs.len());
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
                let ex = e.as_ref().expect("先 objects");
                let k: usize = rest.first().and_then(|s| s.parse().ok()).unwrap_or(4);
                let mut reps = free_reps(ex);
                reps.truncate(k);
                println!(
                    "{{\"round\":{k},\"suggested_labels\":{},\"free_classes\":{}}}",
                    json_ids(&reps),
                    free_reps(ex).len()
                );
            }
            "explain" => explain(e.as_ref().expect("先 objects")),
            "capacity" => {
                let ex = e.as_ref().expect("先 objects");
                println!("2^{} = {}", ex.classes().len(), ex.capacity());
            }
            "quit" => return false,
            other => println!("未知命令: {other}"),
        }
        true
    };

    match args.first().map(|s| s.as_str()) {
        Some(path) => {
            let text = std::fs::read_to_string(path).expect("读不到脚本");
            for line in text.lines() {
                if !run(line, &mut e) {
                    break;
                }
            }
        }
        None => {
            println!("metaform：输入命令，回车执行（quit 退出）");
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














