//! Integration check for the persisted `--define` form of `state add`.
//!
//! A defined predicate is stored as `{op, operands}` in `.metaform/state.json`, and every load
//! recomputes its extension from the operands and the current object count. `state show` exposes the
//! recomputed state, so this test can observe the same value `state need` uses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "metaform-state-defined-{}-{tag}",
        std::process::id()
    ));
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

fn find<'a>(state: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    state["predicates"]
        .as_array()
        .expect("predicates array")
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("predicate `{name}` missing from {state}"))
}

#[test]
fn define_flag_stores_the_definition_and_reload_recomputes_it() {
    let dir = temp_dir("recompute");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&["state", "add", "--project", project, "--name", "a", "--ids", "0"]);
    run_ok(&["state", "add", "--project", project, "--name", "b", "--ids", "0,1"]);

    let added = json(&run_ok(&[
        "state", "add", "--project", project, "--name", "p", "--define", "all-of:a,b",
    ]));
    assert_eq!(added["ok"], true, "{added}");
    assert_eq!(added["name"], "p", "{added}");
    assert_eq!(added["ids"], serde_json::json!([0]), "{added}");

    let saved = json(&std::fs::read_to_string(state_file(&dir)).expect("state file"));
    let p = find(&saved, "p");
    assert_eq!(p["definition"]["op"], "all_of", "{saved}");
    assert_eq!(p["definition"]["operands"], serde_json::json!(["a", "b"]), "{saved}");
    assert_eq!(p["ids"], serde_json::json!([0]), "{saved}");

    // The project grows to three objects and both operands now also cover object 2.
    let mut doc: serde_json::Value = saved;
    doc["n"] = serde_json::json!(3);
    for name in ["a", "b"] {
        for pred in doc["predicates"].as_array_mut().expect("predicates array") {
            if pred["name"] == name {
                pred["ids"] = serde_json::json!([0, 2]);
            }
        }
    }
    std::fs::write(state_file(&dir), serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    // `state show` loads the file, so it sees the recomputed extension that `state need` also uses.
    let shown = json(&run_ok(&["state", "show", "--project", project]));
    assert_eq!(
        find(&shown, "p")["ids"],
        serde_json::json!([0, 2]),
        "the stale [0] mask must be recomputed over three objects: {shown}"
    );
    let need = json(&run_ok(&["state", "need", "--project", project]));
    assert!(need["status"].is_string(), "{need}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ids_flag_still_stores_an_extensional_predicate() {
    let dir = temp_dir("extensional");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "4"]);
    let added = json(&run_ok(&[
        "state", "add", "--project", project, "--name", "p", "--ids", "0,2",
    ]));
    assert_eq!(added["ids"], serde_json::json!([0, 2]), "{added}");
    let saved = json(&std::fs::read_to_string(state_file(&dir)).expect("state file"));
    assert_eq!(find(&saved, "p")["ids"], serde_json::json!([0, 2]), "{saved}");
    assert!(find(&saved, "p").get("definition").is_none(), "{saved}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_operand_is_a_clear_error_and_does_not_write_the_predicate() {
    let dir = temp_dir("missing-operand");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&["state", "add", "--project", project, "--name", "a", "--ids", "0"]);

    let out = run(&[
        "state", "add", "--project", project, "--name", "p", "--define", "all-of:a,ghost",
    ]);
    assert!(!out.status.success(), "a missing operand must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ghost"), "the error must name the operand: {stderr}");

    let saved = json(&std::fs::read_to_string(state_file(&dir)).expect("state file"));
    assert!(
        saved["predicates"].as_array().expect("predicates").iter().all(|p| p["name"] != "p"),
        "the rejected predicate must not be written: {saved}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
