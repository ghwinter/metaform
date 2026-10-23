//! End-to-end check of the new-atom loop, with no model in the loop.
//!
//! `suggest` is read-only and proves that the current vocabulary cannot separate a colliding pair:
//! every `all_of` / `any_of` candidate is constant on the pair (Theorem 11), so `suggestions` is empty
//! and `reason` says why. The only way out is a predicate outside the current atoms. This test acts
//! as the external proposer: it writes a `metaform.propose/1` request and response itself, lets
//! `metaform propose` validate the new extensional predicate, adopts the accepted extension through
//! `state add` exactly as the CLI reports it, and checks that `state need` no longer sees the pair.
//!
//! The first test uses the documented fixture (`red`, `round`, `small`; object 1 positive, object 2
//! negative). That state has exactly one colliding pair, and both of its members were asked
//! directly, so `suggest` recommends a new atom (`new_atom`): asking cannot remove the pair and
//! there is no unasked member left to probe. The vocabulary still cannot separate the pair, and the
//! adopted new atom is what removes it. The second test covers the many-pair case: three colliding
//! pairs from a 1-vs-3 vote also make `suggest` report `next: "new_atom"`, and the same
//! propose -> adopt -> need sequence resolves all three.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

const REQUEST: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "new-atom-loop",
  "objects": 4,
  "vocabulary": [
    {"name": "red", "ids": [0, 1, 2]},
    {"name": "round", "ids": [1, 2, 3]},
    {"name": "small", "ids": [0]}
  ],
  "evidence": {"pos": [1], "neg": [2]},
  "limit": 8
}"#;

const RESPONSE: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "new-atom-loop",
  "candidates": [
    {"name": "mid", "ids": [1], "warrant": "object 1 is the only object with this attribute", "falsifier": "object 2 also has it"}
  ]
}"#;

const REQUEST_THREE: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "new-atom-three",
  "objects": 4,
  "vocabulary": [],
  "evidence": {"pos": [0], "neg": [1, 2, 3]},
  "limit": 8
}"#;

const RESPONSE_THREE: &str = r#"{
  "protocol": "metaform.propose/1",
  "request_id": "new-atom-three",
  "candidates": [
    {"name": "only0", "ids": [0], "warrant": "object 0 is the positive", "falsifier": "any negative is 0"}
  ]
}"#;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-new-atom-{}-{tag}", std::process::id()));
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

fn state_bytes(project: &Path) -> Vec<u8> {
    std::fs::read(state_file(project)).expect("read state")
}

/// Run `suggest` and assert it did not change `.metaform/state.json` byte-for-byte.
fn suggest_read_only(project: &str) -> serde_json::Value {
    let before = state_bytes(Path::new(project));
    let report = json(&run_ok(&["suggest", "--project", project]));
    let after = state_bytes(Path::new(project));
    assert_eq!(
        before, after,
        "suggest must leave .metaform/state.json byte-for-byte unchanged"
    );
    report
}

#[test]
fn proposed_new_atom_resolves_the_collision() {
    let dir = temp_dir("loop");
    let project = dir.to_str().expect("utf-8 path");

    run_ok(&["state", "init", "--project", project, "--n", "4"]);
    for (name, ids) in [("red", "0,1,2"), ("round", "1,2,3"), ("small", "0")] {
        run_ok(&[
            "state",
            "add",
            "--project",
            project,
            "--name",
            name,
            "--ids",
            ids,
        ]);
    }
    run_ok(&[
        "state",
        "vote",
        "--project",
        project,
        "--id",
        "1",
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
        "2",
        "--yes",
        "0",
        "--no",
        "3",
    ]);

    // Step 1: the vocabulary is proven insufficient; the pair (1,2) collides.
    let before = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(before["action"], "PROPOSE", "{before}");
    assert_eq!(
        before["colliding_pairs"],
        serde_json::json!([[1, 2]]),
        "{before}"
    );

    // Step 2: the Boolean search finds nothing and names the next move. Both members of the
    // colliding class were asked, so no probe is left and the recommendation is `new_atom`; the
    // read-only check covers the first `suggest` run.
    let report = suggest_read_only(project);
    assert_eq!(report["status"], "INSUFFICIENT", "{report}");
    assert_eq!(
        report["reason"], "boolean_recombination_cannot_separate",
        "{report}"
    );
    assert_eq!(
        report["suggestions"],
        serde_json::json!([]),
        "Boolean recombination cannot break the collision: {report}"
    );
    assert_eq!(
        report["colliding_pairs"],
        serde_json::json!([[1, 2]]),
        "{report}"
    );
    assert_eq!(
        report["next"], "new_atom",
        "both members of the colliding class were asked, so no probe is left: {report}"
    );
    assert!(
        report.get("ask").is_none(),
        "new_atom must omit the ask field entirely: {report}"
    );
    assert_eq!(
        report["tried"], 9,
        "3 singletons + 3 conjunctions + 3 disjunctions: {report}"
    );

    // Step 3: act as the external proposer. `mid = {1}` separates object 1 from object 2, and no
    // Boolean combination of `red`, `round`, `small` can do that.
    let request = dir.join("request.json");
    let response = dir.join("response.json");
    std::fs::write(&request, REQUEST).expect("write request");
    std::fs::write(&response, RESPONSE).expect("write response");

    let verdict = json(&run_ok(&[
        "propose",
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]));
    assert_eq!(
        verdict["accepted"],
        serde_json::json!([{
            "name": "mid",
            "ids": [1],
            "splits": 1,
            "separates": [[1, 2]],
            "still_colliding": [],
            "warrant": "object 1 is the only object with this attribute",
            "falsifier": "object 2 also has it"
        }]),
        "the new atom must be accepted and split the colliding pair: {verdict}"
    );
    assert_eq!(verdict["rejected"], serde_json::json!([]), "{verdict}");

    // Step 4: adopt the accepted candidate exactly as the CLI reports it.
    let name = verdict["accepted"][0]["name"]
        .as_str()
        .expect("accepted name")
        .to_string();
    let ids: Vec<String> = verdict["accepted"][0]["ids"]
        .as_array()
        .expect("accepted ids")
        .iter()
        .map(|id| id.as_u64().expect("integer id").to_string())
        .collect();
    let ids_arg = ids.join(",");
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        &name,
        "--ids",
        &ids_arg,
    ]);

    let after = json(&run_ok(&["state", "need", "--project", project]));
    assert_ne!(after["status"], "INSUFFICIENT", "{after}");
    assert!(
        after["status"] == "IDENTIFIED" || after["status"] == "UNDETERMINED",
        "the pair is gone, so the state may be identified or still open: {after}"
    );
    let pairs = after["colliding_pairs"]
        .as_array()
        .expect("colliding_pairs array");
    assert!(
        !pairs.iter().any(|pair| pair == &serde_json::json!([1, 2])),
        "the specific collision (1,2) must be resolved: {after}"
    );
    assert_eq!(after["colliding_pairs"], serde_json::json!([]), "{after}");

    // Step 5: `suggest` is read-only after the adoption too; the state file changed only because
    // `state add` wrote it, never because either `suggest` run touched it.
    let report_after = suggest_read_only(project);
    assert_eq!(report_after["status"], "UNDETERMINED", "{report_after}");
    assert_eq!(report_after["colliding"], 0, "{report_after}");
    assert_eq!(
        report_after["suggestions"],
        serde_json::json!([]),
        "{report_after}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The many-pair `new_atom` branch: a fully probed 1-vs-3 vote yields three colliding pairs, so
/// `suggest` names `new_atom` -- the old count rule and the new rule agree here. Adopting one new
/// extensional predicate resolves all three pairs.
#[test]
fn three_colliding_pairs_recommend_new_atom() {
    let dir = temp_dir("new-atom");
    let project = dir.to_str().expect("utf-8 path");

    run_ok(&["state", "init", "--project", project, "--n", "4"]);
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
    for id in ["1", "2", "3"] {
        run_ok(&[
            "state",
            "vote",
            "--project",
            project,
            "--id",
            id,
            "--yes",
            "0",
            "--no",
            "3",
        ]);
    }

    let before = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(
        before["colliding_pairs"],
        serde_json::json!([[0, 1], [0, 2], [0, 3]]),
        "{before}"
    );

    let report = suggest_read_only(project);
    assert_eq!(
        report["reason"], "boolean_recombination_cannot_separate",
        "{report}"
    );
    assert_eq!(report["suggestions"], serde_json::json!([]), "{report}");
    assert_eq!(
        report["next"], "new_atom",
        "three colliding pairs need a predicate outside the current atoms: {report}"
    );
    assert_eq!(report["colliding"], 3, "{report}");

    let request = dir.join("request.json");
    let response = dir.join("response.json");
    std::fs::write(&request, REQUEST_THREE).expect("write request");
    std::fs::write(&response, RESPONSE_THREE).expect("write response");
    let verdict = json(&run_ok(&[
        "propose",
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]));
    assert_eq!(
        verdict["accepted"],
        serde_json::json!([{
            "name": "only0",
            "ids": [0],
            "splits": 3,
            "separates": [[0, 1], [0, 2], [0, 3]],
            "still_colliding": [],
            "warrant": "object 0 is the positive",
            "falsifier": "any negative is 0"
        }]),
        "{verdict}"
    );

    let name = verdict["accepted"][0]["name"]
        .as_str()
        .expect("accepted name")
        .to_string();
    let ids: Vec<String> = verdict["accepted"][0]["ids"]
        .as_array()
        .expect("accepted ids")
        .iter()
        .map(|id| id.as_u64().expect("integer id").to_string())
        .collect();
    let ids_arg = ids.join(",");
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        &name,
        "--ids",
        &ids_arg,
    ]);

    let after = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(after["status"], "IDENTIFIED", "{after}");
    assert_eq!(after["action"], "STOP", "{after}");
    assert_eq!(
        after["colliding_pairs"],
        serde_json::json!([]),
        "the accepted new atom separates every colliding pair: {after}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
