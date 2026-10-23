//! Repository checks that used to be Python scripts.
//!
//! Three of the gates were small file scans living in `scripts/*.py`, which meant the release path
//! needed an interpreter that is not part of the product. They are here now, in the same binary that
//! everything else ships in: `metaform check all` runs them and exits non-zero if any of them fails.
//!
//! The rules themselves are unchanged:
//!
//!   prints   no kernel module writes to a stream; a print anywhere else is counted, so the exception
//!            stays visible instead of becoming the norm
//!   layers   an engine module may only import a layer at or below its own, and an unclassified
//!            module is a failure. This scan sees files in this crate's `src` only: the measure
//!            (`metaform-kernel`) and the protocol (`metaform-propose`) are crates now, so the
//!            compiler enforces their position and there is nothing here to check
//!   tracked  only formal documents are tracked; research notes belong in untracked directories

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const KERNEL: &[&str] = &["lib.rs", "state.rs", "operators.rs", "metrics.rs", "coverage.rs"];
const RUNTIME: &[&str] = &[
    "suggest.rs", "explore.rs", "apply.rs", "answer.rs", "laws.rs", "predictions.rs", "plan.rs",
    "ledger.rs", "portfolio.rs", "report.rs", "score.rs", "morphism.rs",
];
const INTERFACE: &[&str] = &["main.rs", "mcp.rs", "check.rs"];

fn tier(name: &str) -> Option<u8> {
    if KERNEL.contains(&name) {
        Some(0)
    } else if RUNTIME.contains(&name) {
        Some(1)
    } else if INTERFACE.contains(&name) {
        Some(2)
    } else {
        None
    }
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine lives under the workspace root")
        .to_path_buf()
}

/// `println!` outside this module is a violation; `eprintln!` is counted.
pub fn prints() -> Result<String, String> {
    // The kernel does no I/O at all: a kernel module that prints is a layering error, not a style
    // choice. Runtime and interface modules may print -- several of them own a `cmd` -- and their
    // sites are counted so the exception stays visible.
    let mut violations = Vec::new();
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    let entries = std::fs::read_dir(src_dir()).map_err(|e| format!("cannot read src: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("cannot read src entry: {e}"))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name == "check.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        // A `cmd` wrapper is CLI glue that happens to live beside its module. Its prints are
        // counted as `cmd` sites rather than failures; everything outside one must be silent.
        let mut cmd_depth: Option<i32> = None;
        for (number, line) in text.lines().enumerate() {
            // `eprintln!` contains `println!`; the character before the match tells them apart.
            // Remove every `eprintln!` first: the substring `println!` inside it is not a bare print.
            let stripped = line.replace("eprintln!", "");
            let bare = stripped.contains("println!");
            let in_cmd = cmd_depth.is_some_and(|d| d > 0);
            if line.contains("fn cmd") {
                cmd_depth = Some(0);
            }
            if let Some(depth) = cmd_depth.as_mut() {
                *depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            }
            if bare {
                if tier(&name) == Some(0) && !in_cmd {
                    violations.push(format!("{name}:{}: a kernel module must not print", number + 1));
                } else {
                    *counted.entry(name.clone()).or_default() += 1;
                }
            }
        }
    }
    if !violations.is_empty() {
        return Err(violations.join("\n"));
    }
    let detail = if counted.is_empty() {
        "none".to_string()
    } else {
        counted.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
    };
    Ok(format!("no print outside a cmd block; print sites: {detail}"))
}
/// An import may only point inward; an unclassified module fails.
pub fn layers() -> Result<String, String> {
    let mut problems = Vec::new();
    let mut classified = 0usize;
    let entries = std::fs::read_dir(src_dir()).map_err(|e| format!("cannot read src: {e}"))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("cannot read src entry: {e}"))?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(own) = tier(&name) else {
            problems.push(format!("{name}: no tier declared in engine/src/check.rs"));
            continue;
        };
        classified += 1;
        let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        for line in text.lines() {
            let line = line.trim_start();
            let Some(rest) = line.strip_prefix("use crate::") else { continue };
            let head: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '{')
                .collect();
            let heads: Vec<String> = if head.starts_with('{') {
                rest.trim_start_matches('{')
                    .split('}')
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(|part| part.trim().split("::").next().unwrap_or("").to_string())
                    .filter(|h| h.chars().next().is_some_and(|c| c.is_ascii_lowercase()))
                    .collect()
            } else {
                vec![head]
            };
            for head in heads {
                let used = format!("{head}.rs");
                match tier(&used) {
                    Some(used_tier) if used_tier > own => problems.push(format!(
                        "{name} imports {head} above its layer"
                    )),
                    Some(_) => {}
                    None => {
                        // A lowercase name with no module file is a type re-exported by lib.rs.
                        if Path::new(&src_dir().join(&used)).exists() {
                            problems.push(format!("{name}: unclassified module {head}"));
                        }
                    }
                }
            }
        }
    }
    if problems.is_empty() {
        Ok(format!("{classified} files classified, no outward imports"))
    } else {
        Err(problems.join("\n"))
    }
}

/// Tracked prose must be a formal document.
pub fn tracked() -> Result<String, String> {
    const ALLOWED: &[&str] = &[
        "README.md",
        "README.zh.md",
        "CHARTER.md",
        "CHARTER.zh.md",
        "experiments/RESULTS.md",
        "experiments/RESULTS.zh.md",
    ];
    const PREFIXES: &[&str] = &[".dsh/skills/", "proofs/", "docs/"];
    let output = Command::new("git")
        .args(["ls-files", "*.md"])
        .current_dir(repo_root())
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err("git ls-files failed".to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut bad = Vec::new();
    let mut total = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let path = line.trim().replace('\\', "/");
        total += 1;
        if ALLOWED.contains(&path.as_str()) || PREFIXES.iter().any(|p| path.starts_with(p)) {
            continue;
        }
        bad.push(path);
    }
    if bad.is_empty() {
        Ok(format!("{total} tracked document(s), all formal"))
    } else {
        Err(format!(
            "tracked prose is not a formal document: {} (move it under research/ or tmp/)",
            bad.join(", ")
        ))
    }
}

pub fn cmd(args: &[String]) -> i32 {
    let mut rest = args.iter().skip(1);
    let which = rest.next().map(String::as_str).unwrap_or("all");
    let checks: Vec<(&str, fn() -> Result<String, String>)> = match which {
        "all" => vec![("prints", prints as fn() -> Result<String, String>), ("layers", layers), ("tracked", tracked)],
        "prints" => vec![("prints", prints)],
        "layers" => vec![("layers", layers)],
        "tracked" => vec![("tracked", tracked)],
        other => {
            eprintln!("unknown check `{other}`; expected prints, layers, tracked, or all");
            return 2;
        }
    };
    let mut failed = false;
    for (name, check) in checks {
        match check() {
            Ok(detail) => println!("[check-{name}] ok: {detail}"),
            Err(problem) => {
                failed = true;
                for line in problem.lines() {
                    println!("[check-{name}] {line}");
                }
            }
        }
    }
    if failed {
        1
    } else {
        0
    }
}
