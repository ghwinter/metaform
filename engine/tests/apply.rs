//! CLI checks for `metaform apply`.
//!
//! `apply` is the one-step move from a filled-in `metaform.propose/1` response to a persisted
//! project state. It must reuse the validator `propose` already trusts, adopt extensional and
//! defined candidates exactly as `state add` would, save once, and leave the state byte-for-byte
//! unchanged whenever the response is rejected as a whole or an adoption fails.
//!
//! The collision fixture is the one the `suggest` tests use: two full predicates put a strong
//! positive and a strong negative in one class, so the state is `INSUFFICIENT`. The request comes
//! from `--emit-request`; the response is written by the test as the external proposer.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-apply-{}-{tag}", std::process::id()));
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

fn json(text: &str) -> Value {
    serde_json::from_str(text.trim()).expect("the engine prints one JSON value")
}

fn state_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("state.json")
}

fn state_bytes(project: &Path) -> Vec<u8> {
    std::fs::read(state_file(project)).expect("read state")
}

fn state(project: &Path) -> Value {
    json(&std::fs::read_to_string(state_file(project)).expect("read state"))
}

fn predicates(project: &Path) -> Vec<Value> {
    state(project)["predicates"]
        .as_array()
        .expect("predicates is an array")
        .clone()
}

fn predicate(project: &Path, name: &str) -> Value {
    predicates(project)
        .into_iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("predicate `{name}` must exist"))
}

/// `n` objects, the given extensional predicates, then object 0 strong-positive and object 1
/// strong-negative. When the predicates are constant on those two objects, the state is
/// `INSUFFICIENT` and `suggest --emit-request` produces a request.
fn collision_fixture(tag: &str, n: usize, preds: &[(&str, &str)]) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    let n_arg = n.to_string();
    run_ok(&["state", "init", "--project", project, "--n", &n_arg]);
    for (name, ids) in preds {
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

/// Emit the request for the fixture and return its path and `request_id`.
fn emit_request(dir: &Path, project: &str) -> (PathBuf, String) {
    let path = dir.join("request.json");
    let out = run_ok(&[
        "suggest",
        "--project",
        project,
        "--emit-request",
        "--out",
        path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(out, "", "--out sends the request to the file");
    let request = json(&std::fs::read_to_string(&path).expect("read request"));
    let id = request["request_id"]
        .as_str()
        .expect("request_id is a string")
        .to_string();
    (path, id)
}

fn write_response(path: &Path, request_id: &str, candidates: &str) {
    std::fs::write(
        path,
        format!(
            "{{\"protocol\":\"metaform.propose/1\",\"request_id\":\"{request_id}\",\"candidates\":[{candidates}]}}"
        ),
    )
    .expect("write response");
}

fn apply(project: &str, request: &Path, response: &Path) -> Output {
    run(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ])
}

/// The documented loop: `suggest --emit-request`, fill the response, `apply` once. The output says
/// what changed, and `state need` afterwards agrees with the `after` block.
#[test]
fn apply_adopts_the_separating_candidate_and_need_agrees() {
    let dir = collision_fixture("collision", 2, &[("a", "0,1"), ("b", "0,1")]);
    let project = dir.to_str().expect("utf-8 path");
    let (request, request_id) = emit_request(&dir, project);
    let response = dir.join("response.json");
    write_response(&response, &request_id, r#"{"name":"only0","ids":[0]}"#);

    let before = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(
        before["colliding_pairs"],
        json!([[0, 1]]),
        "the fixture must start in collision: {before}"
    );

    let out = run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]);
    let want = format!(
        concat!(
            r#"{{"applied":1,"skipped":[],"#,
            r#""before":{{"status":"INSUFFICIENT","colliding":1}},"#,
            r#""after":{{"status":"IDENTIFIED","colliding":0}},"#,
            r#""request_id":"{}"}}"#,
            "\n"
        ),
        request_id
    );
    assert_eq!(out, want, "apply prints one deterministic JSON line");

    let after = json(&run_ok(&["state", "need", "--project", project]));
    assert_eq!(after["status"], "IDENTIFIED", "{after}");
    assert_eq!(after["action"], "STOP", "{after}");
    assert_eq!(
        after["colliding_pairs"],
        json!([]),
        "the accepted atom separates the only colliding pair: {after}"
    );

    let line = json(&out);
    assert_eq!(
        line["after"]["status"], after["status"],
        "the apply line and state need must agree after adoption"
    );
    assert_eq!(line["after"]["colliding"], json!(0), "{line}");
    assert_eq!(
        predicate(&dir, "only0")["ids"],
        json!([0]),
        "the extensional candidate is stored by its ids"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A response can name a predicate the state has and the request did not: the two commands have
/// drifted. `apply` must not overwrite it. It reports the skip, keeps the existing extension, and
/// still adopts the other candidate. A candidate the validator rejects (`a` repeats the request
/// vocabulary) is reported with the validator's reason.
#[test]
fn apply_skips_duplicate_names_and_still_adopts_the_rest() {
    let dir = collision_fixture("duplicate", 2, &[("a", "0,1"), ("b", "0,1")]);
    let project = dir.to_str().expect("utf-8 path");
    let (request, request_id) = emit_request(&dir, project);

    // The request is now stale on purpose: `c` exists in the state but not in the request.
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "c",
        "--ids",
        "1",
    ]);
    let before_c = predicate(&dir, "c")["ids"].clone();
    assert_eq!(before_c, json!([1]));

    let response = dir.join("response.json");
    write_response(
        &response,
        &request_id,
        concat!(
            r#"{"name":"a","ids":[0]},"#,
            r#"{"name":"c","ids":[0]},"#,
            r#"{"name":"only0","ids":[0]}"#
        ),
    );

    let out = run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]);
    let line = json(&out);
    assert_eq!(line["applied"], json!(1), "{line}");
    assert_eq!(
        line["skipped"],
        json!([
            {"name": "a", "reason": "duplicate_name"},
            {"name": "c", "reason": "name_exists"}
        ]),
        "every candidate that was not adopted is reported with its reason: {line}"
    );

    assert_eq!(
        predicate(&dir, "c")["ids"],
        before_c,
        "an existing name must not be overwritten by an accepted candidate"
    );
    assert_eq!(
        predicates(&dir).iter().filter(|p| p["name"] == "c").count(),
        1,
        "the skipped candidate must not be appended"
    );
    assert_eq!(
        predicate(&dir, "only0")["ids"],
        json!([0]),
        "the rest of the response is still adopted"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Wrong request id rejects the whole response: no candidate is adopted and the state file keeps
/// every byte. The printed verdict is the same shape `propose` prints.
#[test]
fn apply_rejects_a_wrong_request_id_without_touching_the_state() {
    let dir = collision_fixture("wrong-id", 2, &[("a", "0,1"), ("b", "0,1")]);
    let project = dir.to_str().expect("utf-8 path");
    let (request, _) = emit_request(&dir, project);
    let response = dir.join("response.json");
    write_response(
        &response,
        "not-this-request",
        r#"{"name":"only0","ids":[0]}"#,
    );

    let before = state_bytes(&dir);
    let out = apply(project, &request, &response);
    assert!(
        !out.status.success(),
        "a wholesale rejection must exit non-zero"
    );
    assert_eq!(out.status.code(), Some(1), "a rejected response is exit 1");

    let verdict = json(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(
        verdict["accepted"],
        json!([]),
        "no candidate may survive a wrong request id: {verdict}"
    );
    assert_eq!(
        verdict["rejected"],
        json!([{"reason": "wrong_request_id"}]),
        "{verdict}"
    );
    assert_eq!(
        state_bytes(&dir),
        before,
        "the state must stay byte-for-byte unchanged"
    );
    assert!(
        !predicates(&dir).iter().any(|p| p["name"] == "only0"),
        "no candidate may be written"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The same inputs must produce the same bytes. Apply once, restore the pre-adoption state, apply
/// the unchanged request and response again, and compare. Reading the two input files is read-only.
#[test]
fn apply_output_is_byte_deterministic_for_the_same_inputs() {
    let dir = collision_fixture("deterministic", 2, &[("a", "0,1"), ("b", "0,1")]);
    let project = dir.to_str().expect("utf-8 path");
    let (request, request_id) = emit_request(&dir, project);
    let response = dir.join("response.json");
    write_response(&response, &request_id, r#"{"name":"only0","ids":[0]}"#);

    let state_before = state_bytes(&dir);
    let request_before = std::fs::read(&request).expect("read request");
    let response_before = std::fs::read(&response).expect("read response");

    let first = run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(first.lines().count(), 1, "one JSON line: {first}");
    let state_after_first = state_bytes(&dir);

    std::fs::write(state_file(&dir), &state_before).expect("restore the pre-adoption state");
    let second = run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]);

    assert_eq!(
        first, second,
        "the same request, response, and state must produce byte-identical output"
    );
    assert_eq!(
        state_after_first,
        state_bytes(&dir),
        "the restored run must land on the same state bytes"
    );
    assert_eq!(
        std::fs::read(&request).expect("read request"),
        request_before,
        "apply must only read the request file"
    );
    assert_eq!(
        std::fs::read(&response).expect("read response"),
        response_before,
        "apply must only read the response file"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A defined candidate is stored by its definition, not by the extension `propose` computed: the
/// state schema then recomputes the extension on load, exactly as `state add --define` does. The
/// response spells the operands `["b","a"]`; adoption canonicalises them, so the stored definition
/// is the same as `all_of:[a,b]`. The emitted request names an unmeasured object (2), so the
/// candidate carries the L11 prediction.
#[test]
fn apply_stores_a_defined_candidate_as_a_definition() {
    let dir = collision_fixture("defined", 3, &[("a", "0,1"), ("b", "0,1")]);
    let project = dir.to_str().expect("utf-8 path");
    let (request, request_id) = emit_request(&dir, project);
    let response = dir.join("response.json");
    write_response(
        &response,
        &request_id,
        r#"{"name":"p","define":{"all_of":["b","a"]},"predict":{"id":2,"label":"pos"}}"#,
    );

    let out = run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]);
    let line = json(&out);
    assert_eq!(line["applied"], json!(1), "{line}");
    assert_eq!(line["skipped"], json!([]), "{line}");
    assert_eq!(
        line["before"],
        json!({"status": "INSUFFICIENT", "colliding": 1}),
        "{line}"
    );
    assert_eq!(
        line["after"],
        json!({"status": "INSUFFICIENT", "colliding": 1}),
        "a Boolean combination of the vocabulary cannot split its own collision: {line}"
    );

    let p = predicate(&dir, "p");
    assert_eq!(
        p["definition"],
        json!({"op": "all_of", "operands": ["a", "b"]}),
        "the definition must be stored in the state schema: {p}"
    );
    assert_eq!(
        p["ids"],
        json!([0, 1]),
        "the extension cache is recomputed from the definition: {p}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Adoption is all-or-nothing. The first candidate would succeed, but the second names an operand
/// the state does not have (the request is stale), so `resolve` fails and no write may happen.
#[test]
fn a_failed_definition_leaves_the_state_unchanged() {
    let dir = temp_dir("atomic");
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

    // The request knows `c`; the state does not. Both candidates validate against the request.
    let request = dir.join("request.json");
    std::fs::write(
        &request,
        concat!(
            r#"{"protocol":"metaform.propose/1","request_id":"atomic","objects":2,"#,
            r#""vocabulary":[{"name":"a","ids":[0,1]},{"name":"c","ids":[1]}],"#,
            r#""evidence":{"pos":[0],"neg":[1]},"limit":8}"#
        ),
    )
    .expect("write request");
    let response = dir.join("response.json");
    write_response(
        &response,
        "atomic",
        concat!(
            r#"{"name":"first","ids":[0]},"#,
            r#"{"name":"p","define":{"all_of":["c"]}}"#
        ),
    );

    let before = state_bytes(&dir);
    let out = apply(project, &request, &response);
    assert!(
        !out.status.success(),
        "a failed adoption must exit non-zero"
    );
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown operand") && stderr.contains('c'),
        "the error must name the missing operand: {stderr}"
    );
    assert_eq!(
        state_bytes(&dir),
        before,
        "a mid-way failure must leave the state byte-for-byte unchanged"
    );

    let persisted = predicates(&dir);
    let names: Vec<&str> = persisted
        .iter()
        .map(|p| p["name"].as_str().expect("name"))
        .collect();
    assert_eq!(
        names,
        vec!["a"],
        "neither the first candidate nor the failing definition may be persisted"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
