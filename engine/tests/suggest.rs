//! CLI checks for `metaform suggest`.
//!
//! `suggest` is the read-only proof step for an `INSUFFICIENT` state. It enumerates the Boolean
//! definitions the vocabulary invites, counts them, and reports that none can separate a colliding
//! pair: every such pair shares a signature, so every Boolean recombination is constant on it
//! (Theorem 11; `README.md`: "Boolean recombination inside the vocabulary cannot break a collision").
//! These tests freeze that result and the read-only boundary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-suggest-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
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

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text.trim()).expect("the engine prints one JSON value")
}

fn state_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("state.json")
}

/// A two-object project whose only two predicates are both true on both objects. The votes make
/// object 0 a strong positive and object 1 a strong negative, so the pair collides.
fn collision_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "a",
        "--ids",
        "0,1",
    ]);
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "b",
        "--ids",
        "0,1",
    ]);
    run_ok(&[
        "state",
        "vote",
        "--project",
        project,
        "--id",
        "0",
        "--yes",
        "3",
        "--no",
        "0",
    ]);
    run_ok(&[
        "state",
        "vote",
        "--project",
        project,
        "--id",
        "1",
        "--yes",
        "0",
        "--no",
        "3",
    ]);
    dir
}

fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read temp dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect();
        entries.sort();
        for path in entries {
            let rel = path
                .strip_prefix(base)
                .expect("path under base")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
            if path.is_dir() {
                walk(base, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn suggest_reports_the_empty_search_and_is_deterministic() {
    let dir = collision_fixture("deterministic");
    let project = dir.to_str().expect("utf-8 path");

    let first = run_ok(&["suggest", "--project", project]);
    let second = run_ok(&["suggest", "--project", project]);
    assert_eq!(
        first, second,
        "the same state must produce byte-identical output"
    );

    let value = json(&first);
    assert_eq!(value["status"], "INSUFFICIENT", "{value}");
    assert_eq!(value["colliding"], 1, "{value}");
    assert_eq!(
        value["reason"], "boolean_recombination_cannot_separate",
        "{value}"
    );
    assert_eq!(
        value["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{value}"
    );
    assert_eq!(
        value["next"], "ask",
        "one colliding pair is the cheap-ask case"
    );
    assert_eq!(
        value["tried"], 4,
        "two singletons + one conjunction + one disjunction: {value}"
    );
    assert_eq!(value["suggestions"], serde_json::json!([]), "{value}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The impossibility is the feature. If a future change makes Boolean recombination split a
/// collision, this assertion fails and the new output must be re-argued against Theorem 11.
#[test]
fn boolean_recombination_cannot_break_a_collision_theorem_11() {
    let dir = collision_fixture("theorem-11");
    let project = dir.to_str().expect("utf-8 path");

    let before = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(
        before["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{before}"
    );
    assert_eq!(before["classes"], 1, "{before}");

    let report = json(&run_ok(&["suggest", "--project", project]));
    assert_eq!(
        report["suggestions"],
        serde_json::json!([]),
        "every colliding pair shares a signature, so no all_of/any_of candidate can split it \
         (Theorem 11; README: Boolean recombination inside the vocabulary cannot break a collision): {report}"
    );

    // Adopt a Boolean definition the enumerator considered and ask again. The signature partition
    // is unchanged, so the collision survives: this is the record, not a workaround.
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "both",
        "--define",
        "all-of:a,b",
    ]);
    let after = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(after["status"], "INSUFFICIENT", "{after}");
    assert_eq!(
        after["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{after}"
    );
    assert_eq!(after["classes"], before["classes"], "{after}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn suggest_is_read_only() {
    let dir = collision_fixture("read-only");
    let project = dir.to_str().expect("utf-8 path");
    let path = state_file(&dir);

    let state_before = std::fs::read(&path).expect("read state");
    let files_before = tree(&dir);
    let output = run_ok(&["suggest", "--project", project]);
    assert!(json(&output)["tried"].as_u64().expect("tried") > 0);
    let state_after = std::fs::read(&path).expect("read state");
    let files_after = tree(&dir);

    assert_eq!(
        state_before, state_after,
        "suggest must leave .metaform/state.json byte-for-byte unchanged"
    );
    assert_eq!(
        files_before, files_after,
        "suggest must not create or remove files"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_insufficient_states_keep_the_compact_shape() {
    let dir = temp_dir("compact");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "p",
        "--ids",
        "0",
    ]);

    let out = run_ok(&["suggest", "--project", project]);
    assert_eq!(
        out,
        "{\"status\":\"UNDETERMINED\",\"colliding\":0,\"suggestions\":[]}\n"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_max_arity_is_rejected() {
    let dir = collision_fixture("bad-arity");
    let project = dir.to_str().expect("utf-8 path");

    for bad in ["0", "two"] {
        let out = run(&["suggest", "--project", project, "--max-arity", bad]);
        assert_eq!(
            out.status.code(),
            Some(2),
            "--max-arity {bad} must be rejected"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--max-arity"),
            "the error must name the flag: {stderr}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
