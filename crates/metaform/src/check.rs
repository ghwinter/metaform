//! Repository checks that used to be scripts in another language.
//!
//! The gates were small file scans living in `scripts/*.py` and one-liners living in `*.ps1`, which
//! meant the release path needed an interpreter and a shell to establish facts about the
//! repository. They are here now, in the same binary that everything else ships in: `metaform check
//! all` runs the argument-free ones and exits non-zero if any of them fails.
//!
//! The rules about the Rust source are unchanged:
//!
//!   prints   no kernel module writes to a stream; a print anywhere else is counted, so the exception
//!            stays visible instead of becoming the norm. Every workspace member is scanned: a
//!            component that prints is speaking to its own user, but the count has to be visible
//!            rather than inferred from the silence of a directory the scan never entered
//!   layers   an engine module may only import a layer at or below its own, an unclassified module
//!            is a failure, and a crate may only depend on a crate below it in [`CRATE_LAYERS`]
//!   tracked  only formal documents are tracked; research notes belong in untracked directories
//!   members  every workspace member lives under `crates/`, so a directory name states a role
//!
//! The rules about the repository's files are in [`crate::inspect`]: `text`, `count`, `fresh` and
//! `absent` take arguments and are run by the claims register rather than by `all`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const KERNEL: &[&str] = &["lib.rs", "operators.rs", "metrics.rs", "coverage.rs"];
const RUNTIME: &[&str] = &[
    "suggest.rs", "explore.rs", "apply.rs", "answer.rs", "plan.rs", "portfolio.rs", "report.rs",
    "score.rs", "morphism.rs",
];
const INTERFACE: &[&str] = &["main.rs", "mcp.rs", "check.rs", "inspect.rs", "concept.rs"];

/// The crate layering, bottom first: a member may depend only on a member listed before it.
///
/// This table is the one thing Cargo does not check. Cargo refuses a cycle, but a cycle is not what
/// a layering error looks like: `runtime -> vsa` compiles fine and puts a specific vector-memory
/// implementation inside the protocol layer that is supposed to be indifferent to which component
/// runs. The layer numbers are written down here so the next edge is refused by the gate rather
/// than by a reviewer's memory.
///
/// Two positions are worth arguing rather than asserting:
///
///   `metaform` is not the top. It holds the library that `runtime` and `components` are built on
///   (`crates/runtime/src/lib.rs`: "The kernel itself (the `metaform` crate) does not depend on
///   this crate or on any component"), and its own manifest depends only on kernel/propose/store.
///   The `INTERFACE`/`tier` table above orders *files inside a crate*, not crates: `main.rs` and
///   `mcp.rs` sit above `suggest.rs` while the crate that contains both sits below `runtime`. Two
///   questions, two tables -- merging them would let either answer hide the other.
///
///   the three engines are above `runtime`, not beside `store`. They depend on nothing, so their
///   height is decided by what may know them, not by what they know: `runtime` defines the proposer
///   protocol without naming an implementation, and `components` is the crate that wires
///   `vsa`/`sme`/`ilp` behind it. Putting them below `runtime` would have made `runtime -> vsa` a
///   legal downward edge, which is precisely the edge that must fail.
///
/// No exception is currently needed, and that is worth stating: an entry here would be a rule the
/// table refuses to express, not a comment.
const CRATE_LAYERS: &[&[&str]] = &[
    &["kernel"],
    &["propose"],
    &["store"],
    &["metaform"],
    &["runtime"],
    &["vsa", "sme", "ilp"],
    &["components"],
];

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

/// The layer of a workspace member, keyed by its directory name under `crates/`.
fn crate_layer(dir: &str) -> Option<u8> {
    CRATE_LAYERS.iter().position(|group| group.contains(&dir)).map(|layer| layer as u8)
}

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `*.rs` file below `dir`, recursively: `src/bin/*.rs` is source too, and a scan that stops
/// at the top level would report a crate as silent because its binaries live one directory down.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("cannot read an entry of {}: {e}", dir.display()))?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// `println!` outside this module is a violation; `eprintln!` is counted.
pub fn prints() -> Result<String, String> {
    // The kernel does no I/O at all: a kernel module that prints is a layering error, not a style
    // choice. Runtime and interface modules may print -- several of them own a `cmd` -- and their
    // sites are counted so the exception stays visible. Other members are libraries and
    // components whose whole job is to report to their caller: their prints are counted per file
    // and never judged, so "vsa prints thirty times" becomes a fact on the output instead of an
    // absence.
    let mut violations = Vec::new();
    let mut counted: BTreeMap<String, usize> = BTreeMap::new();
    for manifest in crate::inspect::manifests()? {
        let Some(root) = manifest.parent() else {
            return Err(format!("manifest {} has no parent directory", manifest.display()));
        };
        let member = root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("cannot name the member living in {}", root.display()))?
            .to_string();
        let mut files = Vec::new();
        rust_files(&root.join("src"), &mut files)?;
        for path in files {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            let key = format!("{member}/{}", rel.strip_prefix("src/").unwrap_or(&rel));
            // This module is the one that prints the check's own report; excluding it keeps the
            // count about the product rather than about the reporter.
            if key == "metaform/check" {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            // `tier` names files of the engine crate only: `crates/vsa/src/lib.rs` is a library root,
            // not the measure, and must not inherit the kernel's silence.
            let engine_kernel =
                member == "metaform" && tier(path.file_name().and_then(|n| n.to_str()).unwrap_or("")) == Some(0);
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
                    if engine_kernel && !in_cmd {
                        violations.push(format!("{key}.rs:{}: a kernel module must not print", number + 1));
                    } else {
                        *counted.entry(key.clone()).or_default() += 1;
                    }
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
        counted.iter().map(|(k, v)| format!("{k}.rs={v}")).collect::<Vec<_>>().join(", ")
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
            problems.push(format!("{name}: no tier declared in crates/metaform/src/check.rs"));
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
        match crate_edges_from_workspace() {
            Ok(edges) => Ok(format!("{classified} files classified, no outward imports; {edges}")),
            Err(edge_problems) => Err(edge_problems),
        }
    } else {
        Err(problems.join("\n"))
    }
}

/// One workspace member as the cross-crate check sees it, so a test can state a layering error
/// without a workspace on disk.
struct Manifest<'a> {
    /// The `[package] name`, which is how another manifest's `[dependencies]` key refers to it.
    package: &'a str,
    /// The directory name under `crates/`: the key into [`CRATE_LAYERS`].
    dir_name: &'a str,
    dir: &'a Path,
    text: &'a str,
}

/// One `[dependencies]` line: the key as written, its `path` value if it has one, and the line
/// number so a failure can point at it.
struct Dep {
    key: String,
    path: Option<String>,
    line: usize,
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn manifest_label(member: &Manifest<'_>) -> String {
    let path = member.dir.join("Cargo.toml");
    // Forward slashes so the message reads the same on every host, and so a test can assert it.
    path.strip_prefix(crate::inspect::repo_root())
        .unwrap_or(&path)
        .display()
        .to_string()
        .replace('\\', "/")
}

/// The `path = "..."` value of an inline dependency table, if the dependency is a path dependency.
fn path_spec(value: &str) -> Option<String> {
    let (start, end) = (value.find('{')?, value.rfind('}')?);
    for part in value[start + 1..end].split(',') {
        let Some((key, raw)) = part.split_once('=') else { continue };
        if key.trim() == "path" {
            return Some(raw.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The `[dependencies]` entries of one manifest.
///
/// A line that cannot be read is an error rather than a skip: a check that treats what it does not
/// understand as compliant is a check that reports the shape of its own parser.
fn deps_of(package: &str, manifest: &str) -> Result<Vec<Dep>, String> {
    let mut deps = Vec::new();
    let mut in_deps = false;
    for (index, raw) in manifest.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            // `[target.'cfg(unix)'.dependencies]` ships, so it counts. `[dev-dependencies]` does
            // not ship: a test that reaches across a layer is not an edge of the product, and the
            // table is about what the built artefact links.
            let section = line.trim_start_matches('[').trim_end_matches(']').trim();
            in_deps = section == "dependencies" || section.ends_with(".dependencies");
            continue;
        }
        if !in_deps || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "{package} Cargo.toml:{}: cannot read the dependency entry: {line}",
                index + 1
            ));
        };
        let key = key.trim();
        // A key with a space or a brace in it is the second half of a broken line, not a
        // dependency: `metaform-vsa { path = "../vsa" }` splits at the `=` inside the table and
        // would otherwise be filed under a package nobody has, then skipped as "not a member".
        if key.is_empty() || key.contains('{') || key.chars().any(char::is_whitespace) {
            return Err(format!(
                "{package} Cargo.toml:{}: cannot read the dependency entry: {line}",
                index + 1
            ));
        }
        deps.push(Dep { key: key.to_string(), path: path_spec(value), line: index + 1 });
    }
    Ok(deps)
}

/// No member may depend on a member at or above its own layer.
fn crate_edges(members: &[Manifest<'_>]) -> Result<String, String> {
    if members.is_empty() {
        // Same empty-set trap as `tracked` and `members`: an empty member list satisfies every
        // statement of the form "each member's dependencies point down".
        return Err("no workspace member to check: cargo metadata reported none (wrong working directory)"
            .to_string());
    }
    let mut by_dir: BTreeMap<PathBuf, &str> = BTreeMap::new();
    let mut by_package: BTreeMap<&str, &str> = BTreeMap::new();
    for member in members {
        by_dir.insert(normalize(member.dir), member.dir_name);
        by_package.insert(member.package, member.dir_name);
    }
    let mut problems = Vec::new();
    let mut edges = 0usize;
    for member in members {
        let Some(own) = crate_layer(member.dir_name) else {
            problems.push(format!(
                "{}: `{}` has no layer in CRATE_LAYERS",
                manifest_label(member),
                member.dir_name
            ));
            continue;
        };
        for dep in deps_of(member.package, member.text)? {
            // A workspace dependency is spelled either by path or by package name (`package = ...`
            // renames are caught by the path). Anything else -- serde, rand -- is outside the table.
            let target = dep
                .path
                .as_deref()
                .and_then(|spec| by_dir.get(&normalize(&member.dir.join(spec))).copied())
                .or_else(|| by_package.get(dep.key.as_str()).copied());
            let Some(target) = target else { continue };
            edges += 1;
            let Some(dep_layer) = crate_layer(target) else {
                problems.push(format!(
                    "{}:{}: `{target}` has no layer in CRATE_LAYERS",
                    manifest_label(member),
                    dep.line
                ));
                continue;
            };
            if dep_layer >= own {
                problems.push(format!(
                    "{}:{}: {} depends on {} at layer {}, not below its own layer {}",
                    manifest_label(member),
                    dep.line,
                    member.package,
                    target,
                    dep_layer,
                    own
                ));
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok(format!("{} member(s), {edges} workspace dependency edge(s), all point down a layer", members.len()))
}

/// The `[package] name` of a manifest, and nothing else that happens to start with `name` (the
/// `[[bin]]` entries each have one too).
fn package_name(manifest: &str) -> Option<&str> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line.trim_start_matches('[').trim_end_matches(']').trim() == "package";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(value) = line.strip_prefix("name").and_then(|rest| rest.trim_start().strip_prefix('=')) {
            return Some(value.trim().trim_matches('"'));
        }
    }
    None
}

fn crate_edges_from_workspace() -> Result<String, String> {
    let manifests = crate::inspect::manifests()?;
    let mut texts = Vec::with_capacity(manifests.len());
    for manifest in &manifests {
        texts.push(
            std::fs::read_to_string(manifest)
                .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?,
        );
    }
    let mut members = Vec::with_capacity(manifests.len());
    for (manifest, text) in manifests.iter().zip(&texts) {
        let dir = manifest
            .parent()
            .ok_or_else(|| format!("manifest {} has no parent directory", manifest.display()))?;
        let dir_name = dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("cannot name the member living in {}", dir.display()))?;
        let package =
            package_name(text).ok_or_else(|| format!("{}: no [package] name", manifest.display()))?;
        members.push(Manifest { package, dir_name, dir, text });
    }
    crate_edges(&members)
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
        .current_dir(crate::inspect::repo_root())
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
    if total == 0 {
        // `git ls-files` run from the wrong directory still succeeds and still lists files -- just
        // none of them. Without this, a wrong `repo_root` turns the rule below into a rule about
        // the empty set, which every tree satisfies. The crate moved one level down once; the next
        // move should fail here rather than pass quietly.
        return Err(format!(
            "git ls-files found no markdown under {}: wrong repository root",
            crate::inspect::repo_root().display()
        ));
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
    let tail: Vec<String> = args.iter().skip(2).cloned().collect();
    let results: Vec<(&str, Result<String, String>)> = match which {
        "all" => vec![
            ("prints", prints()),
            ("layers", layers()),
            ("tracked", tracked()),
            ("members", crate::inspect::members(&[])),
        ],
        "prints" => vec![("prints", prints())],
        "layers" => vec![("layers", layers())],
        "tracked" => vec![("tracked", tracked())],
        "text" => vec![("text", crate::inspect::text(&tail))],
        "count" => vec![("count", crate::inspect::count(&tail))],
        "fresh" => vec![("fresh", crate::inspect::fresh(&tail))],
        "absent" => vec![("absent", crate::inspect::absent(&tail))],
        "members" => vec![("members", crate::inspect::members(&tail))],
        other => {
            eprintln!(
                "unknown check `{other}`; expected prints, layers, tracked, members, text, count, fresh, absent, or all"
            );
            return 2;
        }
    };
    let mut failed = false;
    for (name, outcome) in results {
        match outcome {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A member of a fabricated workspace: `dir` is its path under the repository root, and the
    /// directory name is the key into [`CRATE_LAYERS`].
    fn member<'a>(package: &'a str, dir: &'a str, deps: &'a str) -> Manifest<'a> {
        Manifest {
            package,
            dir_name: dir.rsplit('/').next().expect("a path has a last segment"),
            dir: Path::new(dir),
            text: deps,
        }
    }

    fn manifest(package: &str, deps: &str) -> String {
        format!("[package]\nname = \"{package}\"\n\n[dependencies]\n{deps}")
    }

    #[test]
    fn a_dependency_on_the_same_layer_fails() {
        // `vsa` and `sme` share a layer: neither is entitled to the other's types.
        let vsa = manifest("metaform-vsa", "metaform-sme = { path = \"../sme\" }\n");
        let sme = manifest("metaform-sme", "");
        let outcome = crate_edges(&[member("metaform-vsa", "crates/vsa", &vsa), member("metaform-sme", "crates/sme", &sme)]);
        let problem = outcome.expect_err("a same-layer dependency must be refused");
        assert!(problem.contains("metaform-vsa depends on sme"), "{problem}");
        assert!(problem.contains("not below its own layer 5"), "{problem}");
    }

    #[test]
    fn a_runtime_dependency_on_an_engine_fails() {
        // The edge this table exists for: the protocol would have become a caller of one specific
        // component, and the next component would have been added by editing `runtime` too.
        let runtime = manifest("metaform-runtime", "metaform-vsa = { path = \"../vsa\" }\n");
        let vsa = manifest("metaform-vsa", "");
        let problem = crate_edges(&[
            member("metaform-runtime", "crates/runtime", &runtime),
            member("metaform-vsa", "crates/vsa", &vsa),
        ])
        .expect_err("runtime must not name an engine");
        assert!(problem.contains("metaform-runtime depends on vsa"), "{problem}");
        assert!(problem.contains("not below its own layer 4"), "{problem}");
    }

    #[test]
    fn a_dependency_on_the_layer_above_fails() {
        let runtime = manifest("metaform-runtime", "metaform-components = \"0.6\"\n");
        let components = manifest("metaform-components", "");
        let outcome = crate_edges(&[
            member("metaform-runtime", "crates/runtime", &runtime),
            member("metaform-components", "crates/components", &components),
        ]);
        let problem = outcome.expect_err("a dependency upward must be refused");
        assert!(problem.contains("metaform-runtime depends on components"), "{problem}");
        // The file and line have to be in the message: a gate that says "a layering error exists"
        // sends the reader looking through nine manifests.
        assert!(problem.contains("crates/runtime/Cargo.toml:5"), "{problem}");
    }

    #[test]
    fn a_legal_layering_passes() {
        // The real shape: components at the top, built on runtime, the measure and the three engines.
        let components = manifest(
            "metaform-components",
            "metaform = { path = \"../metaform\" }\nmetaform-runtime = { path = \"../runtime\" }\nmetaform-vsa = { path = \"../vsa\" }\n",
        );
        let runtime = manifest("metaform-runtime", "metaform = { path = \"../metaform\" }\n");
        let metaform = manifest("metaform", "metaform-kernel = { path = \"../kernel\" }\n");
        let vsa = manifest("metaform-vsa", "");
        let kernel = manifest("metaform-kernel", "serde = \"1\"\n");
        let outcome = crate_edges(&[
            member("metaform-components", "crates/components", &components),
            member("metaform-runtime", "crates/runtime", &runtime),
            member("metaform", "crates/metaform", &metaform),
            member("metaform-vsa", "crates/vsa", &vsa),
            member("metaform-kernel", "crates/kernel", &kernel),
        ])
        .expect("five members, every edge downward");
        assert!(outcome.contains("5 member(s)"), "{outcome}");
        assert!(outcome.contains("5 workspace dependency edge(s)"), "{outcome}");
    }

    #[test]
    fn an_empty_member_list_fails() {
        // Every member of the empty set points at a lower layer. The crate moved once and the
        // wrong directory silently turned `tracked` into a statement about nothing; this rule gets
        // the same guard before it can repeat that.
        let outcome = crate_edges(&[]);
        assert!(outcome.is_err(), "no members is a wrong directory, not a clean layering");
    }

    #[test]
    fn a_dependency_line_that_cannot_be_read_fails() {
        let broken = manifest("metaform-runtime", "metaform-components { path = \"../components\" }\n");
        let problem = crate_edges(&[member("metaform-runtime", "crates/runtime", &broken)])
            .expect_err("a line the parser cannot read must not count as compliant");
        assert!(problem.contains("cannot read the dependency entry"), "{problem}");
    }

    #[test]
    fn only_shipped_dependencies_are_edges() {
        // A dev-dependency and a commented-out dependency may not be mistaken for product edges,
        // or the table would start refusing tests instead of architectures.
        let text = "[package]\nname = \"metaform-runtime\"\n\n[dependencies]\nserde_json = \"1\"\n\n[dev-dependencies]\nmetaform-components = { path = \"../components\" }\n# metaform-vsa = { path = \"../vsa\" }\n";
        let deps = deps_of("metaform-runtime", text).expect("the manifest parses");
        assert_eq!(deps.len(), 1, "{:?}", deps.iter().map(|d| &d.key).collect::<Vec<_>>());
        assert_eq!(deps[0].key, "serde_json");
    }

    #[test]
    fn a_path_dependency_is_resolved_relative_to_its_own_manifest() {
        // `../sme` from `crates/vsa` is `crates/sme`; comparing raw strings would miss it.
        let resolved = normalize(&Path::new("crates/vsa").join("../sme"));
        assert_eq!(resolved, PathBuf::from("crates/sme"));
    }

    #[test]
    fn every_workspace_member_has_a_layer() {
        // The table is the only place a crate's position is written down, so a tenth crate must
        // fail here rather than be silently exempt from the rule.
        let manifests = crate::inspect::manifests().expect("cargo metadata at the workspace root");
        assert!(!manifests.is_empty(), "no member to classify");
        for manifest in manifests {
            let dir = manifest.parent().expect("a manifest lives in a directory");
            let name = dir.file_name().and_then(|n| n.to_str()).expect("a named member");
            assert!(crate_layer(name).is_some(), "crates/{name} has no layer in CRATE_LAYERS");
        }
    }
}
