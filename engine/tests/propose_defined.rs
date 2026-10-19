//! Integration check for the `define` form of `metaform.propose/1`: once a definition is accepted,
//! the engine's `need` sees the adopted extension and can stop.
//!
//! A definition is a Boolean combination of predicates that already exist in the request vocabulary,
//! so it is constant on the classes those predicates induce: a definition can never split a pair that
//! collides under *all* of its operands. This check therefore keeps one operand (`b`) in the proposal
//! request only. The state below adopts `a` and the accepted extension `p = a & b = {0}`: before
//! adoption, `a` puts positive 0 and negative 1 in one class (INSUFFICIENT); after adoption, `p`
//! separates them and `need` reports IDENTIFIED / STOP.

use std::path::PathBuf;
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

const REQUEST: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "defined-need",
  "objects": 2,
  "vocabulary": [
    {"name": "a", "ids": [0, 1]},
    {"name": "b", "ids": [0]}
  ],
  "evidence": {"pos": [0], "neg": [1]},
  "limit": 8
}"#;

const RESPONSE: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "defined-need",
  "candidates": [
    {"name": "p", "define": {"all_of": ["a", "b"]}, "warrant": "a and b", "falsifier": "object 1"}
  ]
}"#;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "metaform-propose-defined-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run(args: &[&str]) -> String {
    let out = Command::new(ENGINE)
        .args(args)
        .output()
        .expect("run the metaform binary");
    assert!(
        out.status.success(),
        "`metaform {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("engine stdout is UTF-8")
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text.trim()).expect("the engine prints one JSON line")
}

#[test]
fn accepted_definition_is_visible_to_need() {
    let dir = temp_dir("need");
    let request = dir.join("request.json");
    let response = dir.join("response.json");
    std::fs::write(&request, REQUEST).expect("write request");
    std::fs::write(&response, RESPONSE).expect("write response");

    let accepted = json(&run(&[
        "propose",
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]));
    assert_eq!(
        accepted["accepted"],
        serde_json::json!([{
            "name": "p",
            "ids": [0],
            "splits": 0,
            "warrant": "a and b",
            "falsifier": "object 1"
        }]),
        "the definition must be accepted with its computed extension: {accepted}"
    );
    assert_eq!(accepted["rejected"], serde_json::json!([]), "{accepted}");
    let ids: Vec<String> = accepted["accepted"][0]["ids"]
        .as_array()
        .expect("accepted ids")
        .iter()
        .map(|id| id.as_u64().expect("integer id").to_string())
        .collect();

    // Build the state through the CLI, exactly as an operator would after reading the response.
    let project = dir.to_str().expect("utf-8 path");
    run(&["state", "init", "--project", project, "--n", "2"]);
    run(&["state", "add", "--project", project, "--name", "a", "--ids", "0,1"]);
    run(&["state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0"]);
    run(&["state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3"]);

    let before = json(&run(&["state", "need", "--project", project]));
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(before["action"], "PROPOSE", "{before}");
    assert_eq!(before["classes"], 1, "{before}");
    assert_eq!(before["colliding_pairs"], serde_json::json!([[0, 1]]), "{before}");

    // Adopt the accepted predicate extension, then ask again.
    let ids_arg = ids.join(",");
    run(&["state", "add", "--project", project, "--name", "p", "--ids", &ids_arg]);
    let after = json(&run(&["state", "need", "--project", project]));
    assert_eq!(after["status"], "IDENTIFIED", "{after}");
    assert_eq!(after["action"], "STOP", "{after}");
    assert_eq!(after["classes"], 2, "{after}");
    assert_eq!(after["colliding_pairs"], serde_json::json!([]), "{after}");

    let _ = std::fs::remove_dir_all(&dir);
}
