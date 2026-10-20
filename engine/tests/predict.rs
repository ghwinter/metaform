//! CLI checks for L11: invention is admitted with a held-out prediction.
//!
//! A request that carries `object_ids` names the objects of the held-out test. The objects in that
//! list and not in `evidence` are unmeasured: a candidate on such a request must carry
//! `"predict": {"id": ..., "label": "pos" | "neg"}` before it can be accepted. This suite freezes the
//! four moves of the law:
//!
//! (a) an unmeasured object rejects a candidate without a prediction (`missing_prediction`);
//! (b) the same candidate with a prediction is accepted and the prediction is echoed unchanged;
//! (c) `apply` records adopted predictions in `.metaform/predictions.tsv`, once, and a replay is
//!     idempotent because rows dedupe on `(request_id, name, id)`;
//! (d) `check-predictions` scores a correct and a wrong prediction against the state's strong labels,
//!     leaves an unmeasured one open, and is read-only;
//! (e) a request whose objects are all measured still accepts a prediction-free candidate, so every
//!     pre-L11 request keeps its verdict.
//!
//! The collision fixture is the one `suggest` and `apply` use: two full predicates put a strong
//! positive and a strong negative in one class, so the state is `INSUFFICIENT`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-predict-{}-{tag}", std::process::id()));
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

fn predictions_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("predictions.tsv")
}

fn state_bytes(project: &Path) -> Vec<u8> {
    std::fs::read(state_file(project)).expect("read state")
}

/// Three objects, the vocabulary `a`/`b` true on all of them, object 0 strong-positive, object 1
/// strong-negative. The pair (0, 1) collides and object 2 is unmeasured, so `object_ids` names a
/// held-out test.
fn unmeasured_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "3"]);
    for name in ["a", "b"] {
        run_ok(&[
            "state", "add", "--project", project, "--name", name, "--ids", "0,1,2",
        ]);
    }
    run_ok(&[
        "state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3",
    ]);
    dir
}

/// A request with one unmeasured object (2): `object_ids` lists all three, `evidence` names only 0
/// and 1. `objects` and `object_ids` are the shape `suggest --emit-request` produces.
fn request(request_id: &str) -> String {
    format!(
        concat!(
            r#"{{"protocol":"metaform.propose/1","request_id":"{request_id}","objects":3,"#,
            r#""object_ids":[0,1,2],"vocabulary":[{{"name":"a","ids":[0,1,2]}},{{"name":"b","ids":[0,1,2]}}],"#,
            r#""evidence":{{"pos":[0],"neg":[1]}},"limit":8}}"#
        ),
        request_id = request_id
    )
}

fn response(request_id: &str, candidates: &str) -> String {
    format!(
        "{{\"protocol\":\"metaform.propose/1\",\"request_id\":\"{request_id}\",\"candidates\":[{candidates}]}}"
    )
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write fixture");
    path
}

fn propose(request: &Path, response: &Path) -> Value {
    json(&run_ok(&[
        "propose",
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]))
}

fn apply(project: &str, request: &Path, response: &Path) -> Value {
    json(&run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        request.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ]))
}

/// (a) The held-out object makes a prediction-free candidate unadoptable.
#[test]
fn unmeasured_object_rejects_a_candidate_without_a_prediction() {
    let dir = temp_dir("missing");
    let request = write(&dir, "request.json", &request("predict-missing"));
    let response = write(&dir, "response.json", &response("predict-missing", r#"{"name":"only0","ids":[0]}"#));

    let verdict = propose(&request, &response);
    assert_eq!(verdict["accepted"], json!([]), "{verdict}");
    assert_eq!(
        verdict["rejected"],
        json!([{"name": "only0", "ids": [0], "reason": "missing_prediction"}]),
        "the candidate names no object outside the evidence: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (b) The same candidate with a held-out commitment is accepted, and the response echoes it.
#[test]
fn prediction_is_accepted_and_echoed() {
    let dir = temp_dir("echo");
    let request = write(&dir, "request.json", &request("predict-echo"));
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-echo",
            r#"{"name":"only0","ids":[0],"warrant":"object 0 is the positive","predict":{"id":2,"label":"pos"}}"#,
        ),
    );

    let verdict = propose(&request, &response);
    assert_eq!(verdict["rejected"], json!([]), "{verdict}");
    assert_eq!(
        verdict["accepted"],
        json!([{
            "name": "only0",
            "ids": [0],
            "splits": 1,
            "warrant": "object 0 is the positive",
            "predict": {"id": 2, "label": "pos"}
        }]),
        "the accepted candidate must carry the prediction unchanged: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (c) Apply writes the commitment once; replaying the response with the pre-adoption state restored
/// adopts the same candidate again but must not append a duplicate row.
#[test]
fn apply_records_predictions_once_and_is_idempotent() {
    let dir = unmeasured_fixture("record");
    let project = dir.to_str().expect("utf-8 path");
    let request = write(&dir, "request.json", &request("predict-record"));
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-record",
            r#"{"name":"only0","ids":[0],"predict":{"id":2,"label":"pos"}}"#,
        ),
    );

    let state_before = state_bytes(&dir);
    let first = apply(project, &request, &response);
    assert_eq!(first["applied"], json!(1), "{first}");
    assert_eq!(first["skipped"], json!([]), "{first}");

    let want = concat!(
        "request_id\tname\tid\tpredicted\textension\n",
        "predict-record\tonly0\t2\tpos\t0\n"
    );
    assert_eq!(
        std::fs::read_to_string(predictions_file(&dir)).expect("read predictions"),
        want,
        "one header and one row, deterministic bytes"
    );

    // Replay on the adopted state: the name now exists, so nothing is adopted and nothing is added.
    let second = apply(project, &request, &response);
    assert_eq!(second["applied"], json!(0), "{second}");
    assert_eq!(
        std::fs::read_to_string(predictions_file(&dir)).expect("read predictions"),
        want,
        "a replay on the adopted state must not duplicate the row"
    );

    // Restore the pre-adoption state and replay the very same inputs: the candidate is adopted again,
    // but the `(request_id, name, id)` dedupe keeps the file byte-for-byte identical.
    std::fs::write(state_file(&dir), &state_before).expect("restore the pre-adoption state");
    let third = apply(project, &request, &response);
    assert_eq!(third["applied"], json!(1), "{third}");
    assert_eq!(
        std::fs::read_to_string(predictions_file(&dir)).expect("read predictions"),
        want,
        "re-adopting the same candidate must not duplicate the row"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (d) `check-predictions` scores a correct and a wrong commitment, leaves an unvoted object open,
/// and writes nothing.
#[test]
fn check_predictions_scores_correct_and_wrong_and_is_read_only() {
    let dir = temp_dir("check");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "5"]);
    for name in ["a", "b"] {
        run_ok(&[
            "state", "add", "--project", project, "--name", name, "--ids", "0,1,2,3,4",
        ]);
    }
    run_ok(&[
        "state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3",
    ]);

    // Three accepted predictions: object 2 pos (will be right), object 3 neg (will be wrong),
    // object 4 neg (never voted, so still open).
    let request = write(
        &dir,
        "request.json",
        &format!(
            concat!(
                r#"{{"protocol":"metaform.propose/1","request_id":"{id}","objects":5,"#,
                r#""object_ids":[0,1,2,3,4],"vocabulary":[{{"name":"a","ids":[0,1,2,3,4]}},{{"name":"b","ids":[0,1,2,3,4]}}],"#,
                r#""evidence":{{"pos":[0],"neg":[1]}},"limit":8}}"#
            ),
            id = "predict-check"
        ),
    );
    for (name, ids, predict) in [
        ("p1", "[0]", r#"{"id":2,"label":"pos"}"#),
        ("p2", "[1]", r#"{"id":3,"label":"neg"}"#),
        ("p3", "[2]", r#"{"id":4,"label":"neg"}"#),
    ] {
        let response = write(
            &dir,
            &format!("response-{name}.json"),
            &response(
                "predict-check",
                &format!(r#"{{"name":"{name}","ids":{ids},"predict":{predict}}}"#),
            ),
        );
        let applied = apply(project, &request, &response);
        assert_eq!(applied["applied"], json!(1), "{name}: {applied}");
    }

    // Measure object 2 as positive (p1 right) and object 3 as positive (p2 wrong). Object 4 stays open.
    run_ok(&[
        "state", "vote", "--project", project, "--id", "2", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "3", "--yes", "3", "--no", "0",
    ]);

    let predictions_before = std::fs::read(predictions_file(&dir)).expect("read predictions");
    let state_before = state_bytes(&dir);
    let out = run_ok(&["check-predictions", "--project", project]);
    assert_eq!(
        out,
        concat!(
            r#"{"checked":2,"correct":1,"#,
            r#""wrong":[{"name":"p2","id":3,"predicted":"neg","actual":"pos"}],"open":1}"#,
            "\n"
        ),
        "one deterministic line, scored against the kernel's strong labels"
    );
    assert_eq!(
        std::fs::read(predictions_file(&dir)).expect("read predictions"),
        predictions_before,
        "check-predictions must not rewrite the record"
    );
    assert_eq!(
        state_bytes(&dir),
        state_before,
        "check-predictions must leave the state byte-for-byte unchanged"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (e) No unmeasured object -> the prediction is optional, and the old response shape still passes.
#[test]
fn measured_request_accepts_a_prediction_free_candidate() {
    let dir = temp_dir("measured");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    for name in ["a", "b"] {
        run_ok(&[
            "state", "add", "--project", project, "--name", name, "--ids", "0,1",
        ]);
    }
    run_ok(&[
        "state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3",
    ]);

    // The real emitted request: object_ids [0,1], both objects measured.
    let request_path = dir.join("request.json");
    run_ok(&[
        "suggest",
        "--project",
        project,
        "--emit-request",
        "--out",
        request_path.to_str().expect("utf-8 path"),
    ]);
    let request_json = json(&std::fs::read_to_string(&request_path).expect("read request"));
    assert_eq!(request_json["object_ids"], json!([0, 1]), "{request_json}");
    assert_eq!(
        request_json["evidence"],
        json!({"pos": [0], "neg": [1]}),
        "{request_json}"
    );
    let request_id = request_json["request_id"].as_str().expect("request id");

    let response_path = write(
        &dir,
        "response.json",
        &response(request_id, r#"{"name":"only0","ids":[0]}"#),
    );
    let verdict = propose(&request_path, &response_path);
    assert_eq!(verdict["rejected"], json!([]), "{verdict}");
    assert_eq!(
        verdict["accepted"],
        json!([{"name": "only0", "ids": [0], "splits": 1}]),
        "a fully measured request needs no prediction: {verdict}"
    );
    assert!(
        verdict["accepted"][0].get("predict").is_none(),
        "an absent prediction must not be invented: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
