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
//! The second half of the suite freezes the rival rule. A candidate may name the rule it competes
//! with (`"rival": {"ids": [...]}` or `"rival": {"define": {...}}`); the prediction then has to land
//! on an object where the two extensions disagree, or the candidate is `predict_not_discriminating`.
//! `apply` records the rival's extension in a sixth, trailing column, and `check-predictions` counts
//! the checked bets whose rival disagreed (`discriminating`). A five-column record written before
//! that column existed keeps loading, with an empty rival.
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
            "separates": [[0, 1]],
            "still_colliding": [],
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
        "request_id\tname\tid\tpredicted\textension\trival\tscore\n",
        "predict-record\tonly0\t2\tpos\t0\t\t\n"
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
            r#""wrong":[{"name":"p2","id":3,"predicted":"neg","actual":"pos"}],"open":1,"discriminating":0}"#,
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
        json!([{"name": "only0", "ids": [0], "splits": 1,
            "separates": [[0, 1]], "still_colliding": []}]),
        "a fully measured request needs no prediction: {verdict}"
    );
    assert!(
        verdict["accepted"][0].get("predict").is_none(),
        "an absent prediction must not be invented: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// The rival rule: a bet has to land where two rules disagree.
// ---------------------------------------------------------------------------

/// Three objects whose state vocabulary matches the request exactly: `a` = {0,1}, `b` = {0,2},
/// object 0 strong-positive, object 1 strong-negative. Object 2 is the held-out test, and it is the
/// object `a` and `b` classify differently, so a defined rival built from them can agree or
/// disagree with an ids candidate on purpose.
fn rival_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "3"]);
    run_ok(&[
        "state", "add", "--project", project, "--name", "a", "--ids", "0,1",
    ]);
    run_ok(&[
        "state", "add", "--project", project, "--name", "b", "--ids", "0,2",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3",
    ]);
    dir
}

/// The request over [`rival_fixture`]: `a` = {0,1}, `b` = {0,2}, objects 0 and 1 measured, 2 held out.
fn rival_request(request_id: &str) -> String {
    format!(
        concat!(
            r#"{{"protocol":"metaform.propose/1","request_id":"{request_id}","objects":3,"#,
            r#""object_ids":[0,1,2],"vocabulary":[{{"name":"a","ids":[0,1]}},{{"name":"b","ids":[0,2]}}],"#,
            r#""evidence":{{"pos":[0],"neg":[1]}},"limit":8}}"#
        ),
        request_id = request_id
    )
}

/// (a) A rival whose extension agrees with the candidate on the predicted object makes the bet
/// worthless: the two rules would have made the same prediction there, so `predict.id` does not
/// distinguish them and the candidate is rejected.
#[test]
fn rival_that_agrees_on_the_predicted_object_is_rejected() {
    let dir = temp_dir("rival-agree");
    let request = write(&dir, "request.json", &rival_request("predict-rival-agree"));
    // The candidate covers {0}; `all_of:[a,b]` also covers {0}. On object 2 both say negative.
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-rival-agree",
            concat!(
                r#"{"name":"only0","ids":[0],"predict":{"id":2,"label":"pos"},"#,
                r#""rival":{"define":{"all_of":["a","b"]}}}"#
            ),
        ),
    );

    let verdict = propose(&request, &response);
    assert_eq!(verdict["accepted"], json!([]), "{verdict}");
    assert_eq!(
        verdict["rejected"],
        json!([{"name": "only0", "ids": [0], "reason": "predict_not_discriminating"}]),
        "an agreeing rival must reject the candidate, not merely warn: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (b) A rival that disagrees on the predicted object makes the bet a real test of the difference
/// between the two rules: the candidate is accepted, `apply` adopts it, and the recorded row carries
/// the rival's extension in the new trailing column.
#[test]
fn rival_that_disagrees_is_accepted_and_recorded() {
    let dir = rival_fixture("rival-record");
    let project = dir.to_str().expect("utf-8 path");
    let request = write(&dir, "request.json", &rival_request("predict-rival-record"));
    // The candidate covers {0}; `any_of:[b]` covers {0,2}, so object 2 is where they disagree.
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-rival-record",
            concat!(
                r#"{"name":"only0","ids":[0],"predict":{"id":2,"label":"pos"},"#,
                r#""rival":{"define":{"any_of":["b"]}}}"#
            ),
        ),
    );

    let verdict = propose(&request, &response);
    assert_eq!(verdict["rejected"], json!([]), "{verdict}");
    assert_eq!(
        verdict["accepted"],
        json!([{
            "name": "only0",
            "ids": [0],
            "splits": 0,
            "separates": [],
            "still_colliding": [],
            "predict": {"id": 2, "label": "pos"}
        }]),
        "a disagreeing rival must be accepted: {verdict}"
    );

    let applied = apply(project, &request, &response);
    assert_eq!(applied["applied"], json!(1), "{applied}");
    // The rival column is the rival's extension: the candidate covers 0, the rival covers 0 and 2.
    let want = concat!(
        "request_id\tname\tid\tpredicted\textension\trival\tscore\n",
        "predict-rival-record\tonly0\t2\tpos\t0\t0,2\t\n"
    );
    assert_eq!(
        std::fs::read_to_string(predictions_file(&dir)).expect("read predictions"),
        want,
        "the recorded row must carry the rival extension"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (c) Backward compatibility: a candidate that names no rival is still accepted on a request with
/// an unmeasured object, and its row exists with an empty trailing rival column.
#[test]
fn a_scored_prediction_records_its_score() {
    let dir = unmeasured_fixture("score-column");
    let project = dir.to_str().expect("utf-8 path");
    let request = write(&dir, "request.json", &request("predict-score"));
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-score",
            r#"{"name":"only0","ids":[0],"score":0.5,"predict":{"id":2,"label":"pos"}}"#,
        ),
    );

    let verdict = propose(&request, &response);
    assert_eq!(verdict["rejected"], json!([]), "{verdict}");
    let applied = apply(project, &request, &response);
    assert_eq!(applied["applied"], json!(1), "{applied}");
    let record = std::fs::read_to_string(dir.join(".metaform").join("predictions.tsv"))
        .expect("predictions file");
    assert!(record.ends_with("predict-score\tonly0\t2\tpos\t0\t\t0.5\n"), "{record}");
}
fn candidate_without_a_rival_records_an_empty_rival_column() {
    let dir = unmeasured_fixture("rival-empty");
    let project = dir.to_str().expect("utf-8 path");
    let request = write(&dir, "request.json", &request("predict-no-rival"));
    let response = write(
        &dir,
        "response.json",
        &response(
            "predict-no-rival",
            r#"{"name":"only0","ids":[0],"predict":{"id":2,"label":"pos"}}"#,
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
            "predict": {"id": 2, "label": "pos"}
        }]),
        "no rival means the pre-rival candidate, unchanged: {verdict}"
    );

    let applied = apply(project, &request, &response);
    assert_eq!(applied["applied"], json!(1), "{applied}");
    let want = concat!(
        "request_id\tname\tid\tpredicted\textension\trival\tscore\n",
        "predict-no-rival\tonly0\t2\tpos\t0\t\t\n"
    );
    assert_eq!(
        std::fs::read_to_string(predictions_file(&dir)).expect("read predictions"),
        want,
        "the rival column is present but empty"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (d) A record written before the rival column existed has a five-column header and five fields per
/// row. It must still load: the missing trailing column is read as empty, not as a parse error.
#[test]
fn five_column_predictions_file_written_by_the_old_format_still_scores() {
    let dir = temp_dir("legacy-record");
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&[
        "state", "add", "--project", project, "--name", "a", "--ids", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "0", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "1", "--yes", "0", "--no", "3",
    ]);
    std::fs::write(
        predictions_file(&dir),
        "request_id\tname\tid\tpredicted\textension\nr1\tp\t0\tpos\t0\n",
    )
    .expect("write the old record");

    let out = run_ok(&["check-predictions", "--project", project]);
    assert_eq!(
        out,
        concat!(
            r#"{"checked":1,"correct":1,"wrong":[],"open":0,"discriminating":0}"#,
            "\n"
        ),
        "the old five-column record must score without a parse error"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// (e) `discriminating` counts checked predictions whose row carries a non-empty rival that actually
/// disagreed. Two of the four rows are checked and rivaled (one right, one wrong, and the wrong one
/// is still reported in `wrong`); the rivaled row whose object is unvoted stays open and is not
/// counted.
#[test]
fn check_predictions_counts_discriminating_bets() {
    let dir = temp_dir("discriminating");
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

    let request = write(
        &dir,
        "request.json",
        &format!(
            concat!(
                r#"{{"protocol":"metaform.propose/1","request_id":"{id}","objects":5,"#,
                r#""object_ids":[0,1,2,3,4],"vocabulary":[{{"name":"a","ids":[0,1,2,3,4]}},{{"name":"b","ids":[0,1,2,3,4]}}],"#,
                r#""evidence":{{"pos":[0],"neg":[1]}},"limit":8}}"#
            ),
            id = "predict-discriminating"
        ),
    );
    let candidates = concat!(
        r#"{"name":"p1","ids":[0],"predict":{"id":2,"label":"pos"},"rival":{"ids":[0,2]}},"#,
        r#"{"name":"p2","ids":[1],"predict":{"id":3,"label":"neg"},"rival":{"ids":[0,3]}},"#,
        r#"{"name":"p3","ids":[2],"predict":{"id":4,"label":"neg"}},"#,
        r#"{"name":"p4","ids":[2],"predict":{"id":4,"label":"pos"},"rival":{"ids":[1,4]}}"#
    );
    let response = write(
        &dir,
        "response.json",
        &response("predict-discriminating", candidates),
    );
    let applied = apply(project, &request, &response);
    assert_eq!(applied["applied"], json!(4), "{applied}");

    // p1's bet is right; p2's is wrong; object 4 is never voted, so p3 and p4 stay open.
    run_ok(&[
        "state", "vote", "--project", project, "--id", "2", "--yes", "3", "--no", "0",
    ]);
    run_ok(&[
        "state", "vote", "--project", project, "--id", "3", "--yes", "3", "--no", "0",
    ]);

    let out = run_ok(&["check-predictions", "--project", project]);
    assert_eq!(
        out,
        concat!(
            r#"{"checked":2,"correct":1,"#,
            r#""wrong":[{"name":"p2","id":3,"predicted":"neg","actual":"pos"}],"#,
            r#""open":2,"discriminating":2}"#,
            "\n"
        ),
        "discriminating counts checked rivaled rows, including the wrong one"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
