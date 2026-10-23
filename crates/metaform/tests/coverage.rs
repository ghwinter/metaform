//! CLI checks for `metaform coverage`.
//!
//! The command is the read-only closure over a goal-coverage table: it reports every unknown cell,
//! the cells contradicted by an implication supported by at least two rows, the rows with identical
//! yes/no/? patterns, and the blocking requirements that are still unanswered. These tests freeze
//! those four outputs, the `!` blocking convention, the text format, and the exit-2/no-write rule
//! for a malformed table.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-coverage-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".metaform")).expect("create .metaform");
    dir
}

fn table(tag: &str, text: &str) -> PathBuf {
    let dir = temp_dir(tag);
    std::fs::write(dir.join(".metaform").join("coverage.tsv"), text).expect("write coverage table");
    dir
}

fn run(args: &[&str]) -> Output {
    Command::new(ENGINE)
        .args(args)
        .output()
        .expect("run the metaform binary")
}

fn run_ok(args: &[&str]) -> String {
    let out = run(args);
    assert!(
        out.status.success(),
        "`metaform {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("engine stdout is UTF-8")
}

fn json(text: &str) -> Value {
    serde_json::from_str(text.trim()).expect("the engine prints one JSON value")
}

/// Every path under `dir`, relative to it, sorted. A read-only command must not change this list.
fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect();
        paths.sort();
        for path in paths {
            out.push(
                path.strip_prefix(base)
                    .expect("path under base")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
            if path.is_dir() {
                walk(base, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// A rejected table must exit 2, print nothing to stdout, and leave the project alone.
fn reject(tag: &str, text: &str) -> String {
    let dir = table(tag, text);
    let before = tree(&dir);
    let out = run(&["coverage", "--project", dir.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "`{tag}` should exit 2, got {:?}",
        out.status
    );
    assert!(
        out.stdout.is_empty(),
        "`{tag}` must not print a partial report: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!stderr.trim().is_empty(), "`{tag}` must say why it failed");
    assert_eq!(
        tree(&dir),
        before,
        "`{tag}` must not write into the project"
    );
    stderr
}

#[test]
fn missing_cell_is_reported_once() {
    let dir = table(
        "missing",
        "item\ta\tb\n\
         r1\t?\tyes\n\
         r2\tno\tyes\n",
    );
    let before = tree(&dir);
    let out = run_ok(&["coverage", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["items"], 2);
    assert_eq!(v["requirements"], json!(["a", "b"]));
    assert_eq!(v["missing"], json!([{"item": "r1", "requirement": "a"}]));
    assert_eq!(v["conflicts"], json!([]));
    assert_eq!(v["equivalences"], json!([]));
    assert_eq!(v["blocking"], json!([]));

    let text = run_ok(&[
        "coverage",
        "--project",
        dir.to_str().unwrap(),
        "--format",
        "text",
    ]);
    assert_eq!(text, "r1: a: missing\n");
    assert_eq!(
        tree(&dir),
        before,
        "coverage must not write into the project"
    );
}

#[test]
fn two_supporting_rows_make_an_unknown_a_conflict() {
    let dir = table(
        "conflict",
        "item\ttests\tdocs\n\
         r1\tyes\tyes\n\
         r2\tyes\tyes\n\
         r3\tyes\t?\n",
    );
    let out = run_ok(&["coverage", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["missing"], json!([{"item": "r3", "requirement": "docs"}]));
    let conflicts = v["conflicts"].as_array().expect("conflicts array");
    assert_eq!(conflicts.len(), 1, "one cell, one conflict: {conflicts:?}");
    let conflict = &conflicts[0];
    assert_eq!(conflict["item"], "r3");
    assert_eq!(conflict["requirement"], "docs");
    assert_eq!(conflict["implied_by"], json!(["r1", "r2"]));
    let reason = conflict["reason"].as_str().expect("reason string");
    assert!(reason.contains("r1"), "reason names the support: {reason}");
    assert!(reason.contains("r2"), "reason names the support: {reason}");
    assert!(
        reason.contains("tests"),
        "reason names the premise: {reason}"
    );
    assert!(
        reason.contains("docs"),
        "reason names the conclusion: {reason}"
    );

    let text = run_ok(&[
        "coverage",
        "--project",
        dir.to_str().unwrap(),
        "--format",
        "text",
    ]);
    let line = text
        .lines()
        .find(|line| line.starts_with("r3: docs: implication"))
        .unwrap_or_else(|| panic!("no conflict line in {text:?}"));
    assert!(line.contains("r1") && line.contains("r2"));
}

#[test]
fn single_supporting_row_does_not_make_an_implication() {
    let dir = table(
        "single-support",
        "item\ttests\tdocs\n\
         r1\tyes\tyes\n\
         r2\tyes\t?\n",
    );
    let out = run_ok(&["coverage", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["conflicts"], json!([]), "one support row is not a rule");
    assert_eq!(v["missing"], json!([{"item": "r2", "requirement": "docs"}]));
}

#[test]
fn identical_rows_form_one_equivalence_group() {
    let dir = table(
        "equivalence",
        "item\ta\tb\tc\n\
         r1\tyes\tno\t?\n\
         r2\tyes\tno\t?\n\
         r3\tno\tyes\tyes\n",
    );
    let out = run_ok(&["coverage", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["equivalences"], json!([["r1", "r2"]]));
    assert_eq!(
        v["requirements"],
        json!(["a", "b", "c"]),
        "the header order is preserved"
    );
}

#[test]
fn blocking_requirement_missing_is_reported_but_no_is_not() {
    let dir = table(
        "blocking",
        "item\tprotocol!\tbudget!\tnotes\n\
         r1\t?\tyes\tyes\n\
         r2\tno\tyes\tyes\n",
    );
    let out = run_ok(&["coverage", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(
        v["requirements"],
        json!(["protocol", "budget", "notes"]),
        "the blocking marker is stripped from output names"
    );
    assert_eq!(
        v["blocking"],
        json!([{
            "requirement": "protocol",
            "missing_in": ["r1"],
            "why": "blocking requirement is unanswered"
        }])
    );
    // A `no` is an answer, not a hole: `r2` is not in any `missing_in`, and `budget` is fully
    // answered, so it contributes no blocking entry at all.
    assert_eq!(v["blocking"][0]["missing_in"], json!(["r1"]));
    assert_eq!(
        v["missing"],
        json!([{"item": "r1", "requirement": "protocol"}])
    );
}

#[test]
fn malformed_table_exits_2_with_nothing_written() {
    let bad_cell = reject("bad-cell", "item\ta\nr1\tmaybe\n");
    assert!(bad_cell.contains("maybe"), "name the bad cell: {bad_cell}");
    assert!(bad_cell.contains("`a`"), "name the requirement: {bad_cell}");

    let duplicate = reject("duplicate-requirement", "item\ta\ta\nr1\tyes\tyes\n");
    assert!(
        duplicate.contains("duplicate"),
        "say it is a duplicate: {duplicate}"
    );
    assert!(
        duplicate.contains("`a`"),
        "name the duplicated requirement: {duplicate}"
    );

    let no_header = reject("missing-header", "\n# no header row here\n");
    assert!(
        no_header.contains("header"),
        "say the header is missing: {no_header}"
    );

    // A cell beyond the header names no requirement: that is an unknown column, not a value.
    let unknown = reject("unknown-requirement", "item\ta\nr1\tyes\tyes\n");
    assert!(
        unknown.contains("unknown requirement"),
        "name the unknown column: {unknown}"
    );
}
