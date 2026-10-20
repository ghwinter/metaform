//! CLI checks for `metaform explore`.
//!
//! The command is the read-only port of conexp-clj's `explore-attributes`: it walks the closed sets
//! of the law theory, confirms an implication when the labelled evidence implies it and no object
//! contradicts it, records a refutation with its counterexample object, and records an open question
//! when no evidence object matches the premise. These tests freeze the three outcomes, the
//! deterministic byte-for-byte output, the read-only boundary, and the question budget.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-explore-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn project(tag: &str, state: Value) -> PathBuf {
    let dir = temp_dir(tag);
    let dir_mf = dir.join(".metaform");
    std::fs::create_dir_all(&dir_mf).expect("create .metaform");
    std::fs::write(
        dir_mf.join("state.json"),
        serde_json::to_string_pretty(&state).expect("serialize fixture"),
    )
    .expect("write fixture state");
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

fn state_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("state.json")
}

/// `a = {0}`, `b = {0,1}`, all three objects labelled. Object 0 is the only object with `a`, and it
/// has `b`; object 1 has `b` without `a`, so no object refutes `a -> b`.
fn implication_fixture(tag: &str) -> PathBuf {
    project(
        tag,
        json!({
            "n": 3,
            "predicates": [
                {"name": "a", "ids": [0]},
                {"name": "b", "ids": [0, 1]}
            ],
            "votes": [
                {"id": 0, "yes": 3, "no": 0},
                {"id": 1, "yes": 3, "no": 0},
                {"id": 2, "yes": 0, "no": 3}
            ]
        }),
    )
}

/// `a = {0,2}`, `b = {0}`, objects 0 and 1 labelled. Object 2 has `a` but not `b`, so it refutes
/// the `a -> b` the evidence suggests.
fn refutation_fixture(tag: &str) -> PathBuf {
    project(
        tag,
        json!({
            "n": 3,
            "predicates": [
                {"name": "a", "ids": [0, 2]},
                {"name": "b", "ids": [0]}
            ],
            "votes": [
                {"id": 0, "yes": 3, "no": 0},
                {"id": 1, "yes": 0, "no": 3}
            ]
        }),
    )
}

fn entries(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read temp dir")
            .map(|e| e.expect("dir entry").path())
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

#[test]
fn evidence_implication_is_reported_with_by_evidence() {
    let dir = implication_fixture("implication");
    let out = run_ok(&["explore", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["status"], "IDENTIFIED");
    assert_eq!(v["questions"], 0);
    assert_eq!(
        v["implications"],
        json!([{"premise": ["a"], "conclusion": ["b"], "by": "evidence"}])
    );
    assert_eq!(v["refuted"], json!([]));
    assert_eq!(v["open"], json!([]));
    assert_eq!(v["closed_sets"], 4);
    assert!(v.get("stopped").is_none());
}

#[test]
fn refuted_implication_carries_the_right_counterexample() {
    let dir = refutation_fixture("refutation");
    let out = run_ok(&["explore", "--project", dir.to_str().unwrap()]);
    let v = json(&out);
    assert_eq!(v["status"], "UNDETERMINED");
    assert_eq!(v["refuted"][0]["premise"], json!(["a"]));
    assert_eq!(v["refuted"][0]["conclusion"], json!(["b"]));
    assert_eq!(v["refuted"][0]["counterexample"], 2);
    // The other direction is true in every object that has b, so it becomes a law.
    assert_eq!(v["implications"][0]["premise"], json!(["b"]));
    assert_eq!(v["implications"][0]["conclusion"], json!(["a"]));
}

#[test]
fn output_is_deterministic_and_state_is_untouched() {
    let dir = implication_fixture("read-only");
    let state = state_file(&dir);
    let before_bytes = std::fs::read(&state).expect("read state");
    let before_tree = entries(&dir);

    let first = run_ok(&["explore", "--project", dir.to_str().unwrap()]);
    let second = run_ok(&["explore", "--project", dir.to_str().unwrap()]);

    assert_eq!(first, second, "two runs must print identical bytes");
    assert_eq!(std::fs::read(&state).expect("read state"), before_bytes);
    assert_eq!(
        entries(&dir),
        before_tree,
        "explore must not write into the project"
    );
}

#[test]
fn budget_stops_after_one_premise() {
    let dir = implication_fixture("budget-one");
    let out = run_ok(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--max-questions",
        "1",
    ]);
    let v = json(&out);
    assert_eq!(v["stopped"], "budget");
    assert_eq!(v["budget"], 1);
    assert_eq!(v["closed_sets"], 1);
}

#[test]
fn explosive_family_is_not_materialised() {
    // Eight predicates with no labels: the law theory is empty, so all 2^8 subsets are closed and
    // every premise is an open question. Budget 50 must cut the walk off long before 256.
    let predicates: Vec<Value> = (0..8)
        .map(|i| json!({"name": format!("p{i}"), "ids": [i]}))
        .collect();
    let dir = project(
        "explosive",
        json!({"n": 8, "predicates": predicates, "votes": []}),
    );
    let started = std::time::Instant::now();
    let out = run_ok(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--max-questions",
        "50",
    ]);
    let elapsed = started.elapsed();
    let v = json(&out);
    assert_eq!(v["stopped"], "budget");
    assert_eq!(v["budget"], 50);
    assert_eq!(v["closed_sets"], 50);
    assert_eq!(v["questions"], 50);
    assert_eq!(v["open"].as_array().unwrap().len(), 50);
    assert!(v["implications"].as_array().unwrap().is_empty());
    assert!(elapsed.as_secs() < 5, "the budgeted walk took {elapsed:?}");
}

#[test]
fn out_writes_one_line_and_leaves_stdout_empty() {
    let dir = refutation_fixture("out");
    let path = dir.join("explore.json");
    let before = std::fs::read(state_file(&dir)).expect("read state");
    let out = run_ok(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--out",
        path.to_str().unwrap(),
    ]);
    assert!(out.is_empty(), "stdout must stay empty when --out is used");
    let text = std::fs::read_to_string(&path).expect("read --out file");
    assert_eq!(text.lines().count(), 1, "exactly one JSON line: {text:?}");
    assert!(text.ends_with('\n'));
    let v = json(&text);
    assert_eq!(v["refuted"][0]["counterexample"], 2);
    assert_eq!(std::fs::read(state_file(&dir)).expect("read state"), before);

    // `--out` may not be the state file: that would turn the read-only command into a writer.
    let refused = run(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--out",
        state_file(&dir).to_str().unwrap(),
    ]);
    assert!(
        !refused.status.success(),
        "--out must refuse the state path"
    );
    assert_eq!(std::fs::read(state_file(&dir)).expect("read state"), before);
}
