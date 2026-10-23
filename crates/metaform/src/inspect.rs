//! Assertions about repository files rather than about the Rust source.
//!
//! Nine of the claims the repository makes about itself used to be one-liners written in
//! PowerShell: does a document still contain this string, do two documents agree on a count, is an
//! artefact newer than its source, is a stale build directory absent, does every workspace member
//! live under `crates/`. Each of them ran `pwsh`, so the release path needed a shell to establish a
//! fact about a text file.
//!
//! They are subcommands of `check` now, for the same reason the three source scans were moved here:
//! a claim about the repository has to be a command, and the command has to be part of the product.
//! What the rules keep is what they kept before -- a document may not drift from the number it
//! states, and an artefact may not be older than the source it claims to come from.
//!
//! Every subcommand resolves its paths against the working directory, never against the crate, so a
//! caller chooses the tree it is talking about (the gate is run at the root; the tests run in a
//! temporary directory).
//!
//!   text     a file contains (or does not contain) a literal string
//!   count    the number of lines that begin with (or contain) a marker, or the first number after
//!            one, is N -- optionally the same N as in a second file
//!   fresh    an artefact was written no earlier than its source
//!   absent   a path does not exist (a stale build directory, a dropped artefact)
//!   members  every workspace member lives under `crates/`

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The workspace root, derived from this crate's location rather than from the working directory.
///
/// The `check` gate runs at the root, but `cargo test -p metaform` runs inside `crates/metaform`:
/// a rule about the whole workspace must not change its answer because of where the caller stood.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the engine lives under the workspace root")
        .to_path_buf()
}

/// Positional paths plus the value of each recognised `--flag value`.
///
/// Recognition is explicit and refusing an unknown flag is deliberate: a typo that silently became
/// a path would turn an assertion into a statement about a file nobody reads.
struct Args {
    paths: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    fn parse(args: &[String], valued: &[&str]) -> Result<Self, String> {
        let mut paths = Vec::new();
        let mut flags: Vec<(String, String)> = Vec::new();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            let Some(name) = arg.strip_prefix("--") else {
                paths.push(arg.clone());
                index += 1;
                continue;
            };
            if !valued.contains(&name) {
                return Err(format!("unknown option `--{name}`"));
            }
            match args.get(index + 1) {
                Some(value) if !value.starts_with("--") => {
                    flags.push((name.to_string(), value.clone()));
                    index += 2;
                }
                _ => return Err(format!("--{name} expects a value")),
            }
        }
        Ok(Self { paths, flags })
    }

    fn all(&self, name: &str) -> Vec<&str> {
        self.flags.iter().filter(|(key, _)| key == name).map(|(_, value)| value.as_str()).collect()
    }

    fn one(&self, name: &str) -> Option<&str> {
        self.all(name).first().copied()
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn number(text: &str, flag: &str) -> Result<u64, String> {
    text.parse::<u64>().map_err(|e| format!("{flag} expects a number, got {text:?}: {e}"))
}

/// A file must contain every `--has` string and none of the `--lacks` strings.
pub fn text(args: &[String]) -> Result<String, String> {
    let args = Args::parse(args, &["has", "lacks"])?;
    if args.paths.is_empty() {
        return Err("text expects at least one path".to_string());
    }
    let has = args.all("has");
    let lacks = args.all("lacks");
    if has.is_empty() && lacks.is_empty() {
        return Err("text: give --has <literal> and/or --lacks <literal>".to_string());
    }
    let mut problems = Vec::new();
    for path in &args.paths {
        let body = read(path)?;
        for needle in &has {
            if !body.contains(*needle) {
                problems.push(format!("{path} does not contain {needle:?}"));
            }
        }
        for needle in &lacks {
            if body.contains(*needle) {
                problems.push(format!("{path} contains {needle:?}"));
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok(format!(
        "{} file(s): {} required string(s) present, {} forbidden string(s) absent",
        args.paths.len(),
        has.len(),
        lacks.len()
    ))
}

/// Count lines that begin with a marker or contain one, or read the first number after one.
fn measure(path: &str, marker_kind: &str, marker: &str) -> Result<u64, String> {
    let body = read(path)?;
    match marker_kind {
        "lines-begin" => Ok(body.lines().filter(|line| line.starts_with(marker)).count() as u64),
        "contains" => Ok(body.lines().filter(|line| line.contains(marker)).count() as u64),
        _ => {
            let at = body
                .find(marker)
                .ok_or_else(|| format!("{path} does not contain {marker:?}"))?;
            let rest = &body[at + marker.len()..];
            let digits: String = rest
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if digits.is_empty() {
                return Err(format!("{path}: no number after {marker:?}"));
            }
            number(&digits, marker)
        }
    }
}

/// One measurement, or two measurements that must agree.
pub fn count(args: &[String]) -> Result<String, String> {
    let args = Args::parse(
        args,
        &["lines-begin", "contains", "number-after", "equals", "at-least", "same", "pattern2"],
    )?;
    let kind = match (args.one("lines-begin"), args.one("contains"), args.one("number-after")) {
        (Some(marker), None, None) => ("lines-begin", marker),
        (None, Some(marker), None) => ("contains", marker),
        (None, None, Some(marker)) => ("number-after", marker),
        (None, None, None) => {
            return Err("count: give --lines-begin, --contains or --number-after".to_string())
        }
        _ => {
            return Err(
                "count: --lines-begin, --contains and --number-after are exclusive".to_string()
            )
        }
    };
    let equals = args.one("equals").map(|t| number(t, "--equals")).transpose()?;
    let at_least = args.one("at-least").map(|t| number(t, "--at-least")).transpose()?;

    if let Some(other) = args.one("same") {
        // `--same` names the second file itself, so exactly one path is positional.
        if args.paths.len() != 1 {
            return Err("count --same takes one path before it and the second as its value".to_string());
        }
        // `--equals` alongside `--same` is not a contradiction: it asks for equality *and* for a
        // magnitude, which is how "the charter states thirteen, in both languages" is written.
        let second_marker = args.one("pattern2").unwrap_or(kind.1);
        let left = measure(&args.paths[0], kind.0, kind.1)?;
        let right = measure(other, kind.0, second_marker)?;
        if left != right {
            return Err(format!(
                "{} = {left} but {other} = {right} ({} {second_marker:?})",
                args.paths[0], kind.0
            ));
        }
        if let Some(want) = equals {
            if left != want {
                return Err(format!("both files measure {left}, expected {want}"));
            }
        }
        if let Some(floor) = at_least {
            if left < floor {
                return Err(format!("both files measure {left}, expected at least {floor}"));
            }
        }
        return Ok(format!("{} = {left} and {other} = {right}", args.paths[0]));
    }

    if args.paths.len() != 1 {
        return Err("count takes one path (or one path plus --same <second>)".to_string());
    }
    let path = &args.paths[0];
    let value = measure(path, kind.0, kind.1)?;
    match (equals, at_least) {
        (Some(want), _) if value != want => Err(format!("{path}: {} {value}, expected {want}", kind.0)),
        (_, Some(floor)) if value < floor => {
            Err(format!("{path}: {} {value}, expected at least {floor}", kind.0))
        }
        _ => Ok(format!("{path}: {} {value}", kind.0)),
    }
}

fn modified(path: &str) -> Result<std::time::SystemTime, String> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_err(|e| format!("cannot stat {path}: {e}"))
}

fn seconds(time: std::time::SystemTime) -> String {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "before 1970".to_string())
}

/// The artefact was written no earlier than its source.
pub fn fresh(args: &[String]) -> Result<String, String> {
    let args = Args::parse(args, &["than"])?;
    let source = args.one("than").ok_or("fresh: give --than <source>")?;
    if args.paths.len() != 1 {
        return Err("fresh takes one artefact path and --than <source>".to_string());
    }
    let artefact = &args.paths[0];
    let written = modified(artefact)?;
    let edited = modified(source)?;
    if written < edited {
        return Err(format!(
            "{artefact} is older than {source} ({} < {})",
            seconds(written),
            seconds(edited)
        ));
    }
    Ok(format!("{artefact} is not older than {source} ({})", seconds(written)))
}

/// None of the paths exists.
pub fn absent(args: &[String]) -> Result<String, String> {
    let args = Args::parse(args, &[])?;
    if args.paths.is_empty() {
        return Err("absent expects at least one path".to_string());
    }
    let present: Vec<&str> = args
        .paths
        .iter()
        .filter(|path| Path::new(path).exists())
        .map(String::as_str)
        .collect();
    if !present.is_empty() {
        return Err(format!("present, expected absent: {}", present.join(", ")));
    }
    Ok(format!("{} path(s) absent", args.paths.len()))
}

/// The judgment half of `members`, so a test can hand it a manifest list without running `cargo`.
fn members_from(metadata: &str) -> Result<String, String> {
    let doc: serde_json::Value =
        serde_json::from_str(metadata).map_err(|e| format!("cargo metadata is not JSON: {e}"))?;
    let packages = doc
        .get("packages")
        .and_then(|value| value.as_array())
        .ok_or("cargo metadata has no packages")?;
    if packages.is_empty() {
        // The empty-set trap the tracked rule already fell into once: a wrong working directory
        // still produces a successful, empty answer, and every layout satisfies a rule about it.
        return Err("cargo metadata reported no packages: wrong working directory".to_string());
    }
    let mut outside = Vec::new();
    for package in packages {
        let manifest = package.get("manifest_path").and_then(|value| value.as_str()).unwrap_or("");
        // `<root>/crates/<member>/Cargo.toml`: the directory two levels up from the manifest is the
        // one that has to be called `crates`.
        let under = Path::new(manifest)
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .is_some_and(|name| name == "crates");
        if !under {
            outside.push(if manifest.is_empty() { "<no manifest_path>".to_string() } else { manifest.to_string() });
        }
    }
    if !outside.is_empty() {
        return Err(format!("member(s) outside crates/: {}", outside.join(", ")));
    }
    Ok(format!("{} workspace member(s), all under crates/", packages.len()))
}

/// Every workspace member lives under `crates/`, so the directory name states a role.
pub fn members(args: &[String]) -> Result<String, String> {
    let args = Args::parse(args, &[])?;
    if !args.paths.is_empty() {
        return Err("members takes no arguments".to_string());
    }
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--quiet"])
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    members_from(&String::from_utf8_lossy(&output.stdout))
}

/// The judgment half of [`manifests`], so a test can state the empty-set trap without cargo.
fn manifests_from(metadata: &str) -> Result<Vec<PathBuf>, String> {
    let doc: serde_json::Value =
        serde_json::from_str(metadata).map_err(|e| format!("cargo metadata is not JSON: {e}"))?;
    let packages = doc
        .get("packages")
        .and_then(|value| value.as_array())
        .ok_or("cargo metadata has no packages")?;
    if packages.is_empty() {
        return Err("cargo metadata reported no packages: wrong working directory".to_string());
    }
    let mut manifests = Vec::with_capacity(packages.len());
    for package in packages {
        let manifest = package
            .get("manifest_path")
            .and_then(|value| value.as_str())
            .ok_or("a package in cargo metadata has no manifest_path")?;
        manifests.push(PathBuf::from(manifest));
    }
    Ok(manifests)
}

/// The manifest of every workspace member, from `cargo metadata` run at the repository root.
///
/// Cached for the length of the process: `check all` asks for this list three times (prints,
/// layers, members) and each uncached call is most of a second of cargo start-up. A `check` is a
/// one-shot command, so the list cannot go stale in a way anyone could observe.
pub fn manifests() -> Result<Vec<PathBuf>, String> {
    static CACHE: OnceLock<Result<Vec<PathBuf>, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let output = Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1", "--quiet"])
                .current_dir(repo_root())
                .output()
                .map_err(|e| format!("cannot run cargo: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "cargo metadata failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            manifests_from(&String::from_utf8_lossy(&output.stdout))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_flag_nobody_declared_is_an_error_not_a_path() {
        let parsed = Args::parse(&args(&["doc.md", "--haz", "x"]), &["has"]);
        assert!(parsed.is_err(), "an unknown flag must not become a file name");
    }

    #[test]
    fn a_flag_at_the_end_without_a_value_is_an_error() {
        let parsed = Args::parse(&args(&["doc.md", "--has"]), &["has"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn members_judges_the_directory_two_levels_above_each_manifest() {
        let inside = members_from(
            r#"{"packages":[
                {"name":"a","manifest_path":"crates/a/Cargo.toml"},
                {"name":"b","manifest_path":"crates/b/Cargo.toml"}]}"#,
        )
        .expect("two members under crates");
        assert!(inside.contains("2 workspace member(s)"), "{inside}");

        let outside = members_from(
            r#"{"packages":[{"name":"engine","manifest_path":"engine/Cargo.toml"}]}"#,
        );
        assert!(outside.is_err(), "a member beside crates/ must fail");
    }

    #[test]
    fn members_rejects_an_empty_manifest_list() {
        let empty = members_from(r#"{"packages":[]}"#);
        assert!(empty.is_err(), "no packages means the wrong directory, not a passing rule");
    }

    #[test]
    fn manifests_rejects_an_empty_package_list() {
        let empty = manifests_from(r#"{"packages":[]}"#);
        assert!(empty.is_err(), "a scan over no member would pass on every tree");
    }

    #[test]
    fn manifests_keeps_the_manifest_path_of_each_package() {
        let found = manifests_from(
            r#"{"packages":[
                {"name":"a","manifest_path":"crates/a/Cargo.toml"},
                {"name":"b","manifest_path":"crates/b/Cargo.toml"}]}"#,
        )
        .expect("two manifests");
        assert_eq!(found, vec![PathBuf::from("crates/a/Cargo.toml"), PathBuf::from("crates/b/Cargo.toml")]);
    }
}
