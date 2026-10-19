//! CLI checks for `metaform suggest`.
//!
//! `suggest` is the read-only proof step for an `INSUFFICIENT` state. It enumerates the Boolean
//! definitions the vocabulary invites, counts them, and reports that none can separate a colliding
//! pair: every such pair shares a signature, so every Boolean recombination is constant on it
//! (Theorem 11; `README.md`: "Boolean recombination inside the vocabulary cannot break a collision").
//! These tests freeze that result, the read-only boundary, and the `next` dispatch: `ask` only while
//! a colliding class still has an unasked member; a fully probed class -- even one whose direct
//! labels conflict -- needs a new atom.

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
/// object 0 a strong positive and object 1 a strong negative, so the pair collides. Both members
/// were asked, so the class is fully probed and no question can repair it.
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

fn state_add(project: &str, name: &str, ids: &str) {
    run_ok(&[
        "state", "add", "--project", project, "--name", name, "--ids", ids,
    ]);
}

fn state_vote(project: &str, id: &str, yes: &str, no: &str) {
    run_ok(&[
        "state", "vote", "--project", project, "--id", id, "--yes", yes, "--no", no,
    ]);
}

/// Three objects in one class: 0 is a strong positive, 1 a strong negative, and 2 was never asked.
/// The colliding pair (0,1) is real, but object 2 inherits its label from the class.
fn inherited_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "3"]);
    state_add(project, "a", "0,1,2");
    state_vote(project, "0", "3", "0");
    state_vote(project, "1", "0", "3");
    dir
}

/// Six objects in one class: three strong positives against three strong negatives give nine
/// colliding pairs, and every member was asked, so no question is left to ask.
fn wide_collision_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "6"]);
    state_add(project, "a", "0,1,2,3,4,5");
    for id in ["0", "1", "2"] {
        state_vote(project, id, "3", "0");
    }
    for id in ["3", "4", "5"] {
        state_vote(project, id, "0", "3");
    }
    dir
}

/// Three objects in one class, all asked: 0 is a strong positive, 1 a strong negative, and 2 votes
/// `yes`/`no`, so its side decides the measured pair set. With 2 on either side the class is fully
/// probed and its direct labels conflict.
fn fully_probed_fixture(tag: &str, yes: &str, no: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "3"]);
    state_add(project, "a", "0,1,2");
    state_vote(project, "0", "3", "0");
    state_vote(project, "1", "0", "3");
    state_vote(project, "2", yes, no);
    dir
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
    // Both members were asked, so the old count-based rule's `ask` (one pair) would be a fixed
    // point: another vote cannot remove the pair. The dispatch rule now says `new_atom` instead.
    assert_eq!(
        value["next"], "new_atom",
        "every member of the colliding class was asked: {value}"
    );
    assert!(
        value.get("ask").is_none(),
        "new_atom must omit the ask field entirely: {value}"
    );
    assert_eq!(
        value["tried"], 4,
        "two singletons + one conjunction + one disjunction: {value}"
    );
    assert_eq!(value["suggestions"], serde_json::json!([]), "{value}");

    let _ = std::fs::remove_dir_all(&dir);
}

/// A real collision whose class also holds an unasked member: the question names the unasked
/// member, not one of the two objects that already answered.
#[test]
fn suggest_ask_names_the_inherited_member() {
    let dir = inherited_fixture("ask-inherited");
    let project = dir.to_str().expect("utf-8 path");

    let value = json(&run_ok(&["suggest", "--project", project]));
    assert_eq!(value["status"], "INSUFFICIENT", "{value}");
    assert_eq!(value["next"], "ask", "{value}");
    assert_eq!(
        value["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{value}"
    );

    let ask = &value["ask"];
    assert_eq!(ask["why"], "unprobed_member", "{value}");
    assert_eq!(ask["objects"], serde_json::json!([2]), "{value}");
    assert_eq!(
        ask["objects"].as_array().expect("objects is an array").len() as u64,
        ask["questions"].as_u64().expect("questions is a number"),
        "objects.len() must equal questions: {value}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression: a fully probed conflicting class used to be a fixed point. Under the old count-based
/// rule this state (two colliding pairs, so `<= 2`) reported `next:"ask"` with
/// `why:"conflicting_labels"` and named the class anchor, byte-identically, forever. Every member
/// has now been asked and asking only appends evidence, so the report must recommend the one move
/// that can change the state: a new atom. The third vote is cast on either side, so the test shows
/// that being fully probed -- not the direction of that vote -- decides the recommendation.
#[test]
fn fully_probed_conflict_recommends_new_atom() {
    for (tag, yes, no, pairs) in [
        (
            "fully-probed-positive",
            "3",
            "0",
            serde_json::json!([[0, 1], [2, 1]]),
        ),
        (
            "fully-probed-negative",
            "0",
            "3",
            serde_json::json!([[0, 1], [0, 2]]),
        ),
    ] {
        let dir = fully_probed_fixture(tag, yes, no);
        let project = dir.to_str().expect("utf-8 path");

        let first = run_ok(&["suggest", "--project", project]);
        let second = run_ok(&["suggest", "--project", project]);
        assert_eq!(
            first, second,
            "the same fully probed state must produce byte-identical output"
        );

        let value = json(&first);
        assert_eq!(value["status"], "INSUFFICIENT", "{value}");
        assert_eq!(
            value["reason"], "boolean_recombination_cannot_separate",
            "{value}"
        );
        assert_eq!(value["colliding_pairs"], pairs, "{value}");
        assert_eq!(
            value["next"], "new_atom",
            "no member is left to ask, so the recommendation changed: {value}"
        );
        assert!(
            value.get("ask").is_none(),
            "new_atom must omit the ask field entirely: {value}"
        );
        assert_eq!(value["suggestions"], serde_json::json!([]), "{value}");

        if yes == "3" {
            assert_eq!(
                first,
                concat!(
                    r#"{"status":"INSUFFICIENT","colliding":2,"#,
                    r#""reason":"boolean_recombination_cannot_separate","#,
                    r#""colliding_pairs":[[0,1],[2,1]],"next":"new_atom","tried":1,"suggestions":[]}"#,
                    "\n"
                ),
                "the new recommendation is exact and repeatable"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The fix must not over-correct: while the colliding class still has a member that was never asked
/// directly, that probe is still the question, because its label is inherited rather than tested.
#[test]
fn inherited_label_still_recommends_ask() {
    let dir = inherited_fixture("still-ask");
    let project = dir.to_str().expect("utf-8 path");

    let first = run_ok(&["suggest", "--project", project]);
    let second = run_ok(&["suggest", "--project", project]);
    assert_eq!(
        first, second,
        "the same state must produce byte-identical output"
    );

    let value = json(&first);
    assert_eq!(value["status"], "INSUFFICIENT", "{value}");
    assert_eq!(value["next"], "ask", "{value}");
    assert_eq!(
        value["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{value}"
    );

    let ask = &value["ask"];
    assert_eq!(ask["why"], "unprobed_member", "{value}");
    assert_eq!(ask["questions"], 1, "{value}");
    assert_eq!(
        ask["objects"],
        serde_json::json!([2]),
        "the unasked member is the question: {value}"
    );
    assert_eq!(
        ask["objects"].as_array().expect("objects is an array").len() as u64,
        ask["questions"].as_u64().expect("questions is a number"),
        "objects.len() must equal questions: {value}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A fully probed class leaves no question, so `ask` is omitted rather than `null` -- the same
/// shape as the many-pair case under the old rule.
#[test]
fn suggest_new_atom_omits_ask() {
    let dir = wide_collision_fixture("ask-new-atom");
    let project = dir.to_str().expect("utf-8 path");

    let value = json(&run_ok(&["suggest", "--project", project]));
    assert_eq!(value["status"], "INSUFFICIENT", "{value}");
    assert_eq!(value["next"], "new_atom", "{value}");
    assert_eq!(value["colliding"], 9, "{value}");
    assert!(
        value.get("ask").is_none(),
        "new_atom must omit the ask field entirely: {value}"
    );

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

/// `--emit-request` on an `INSUFFICIENT` state must print one complete, deterministic
/// `metaform.propose/1` request -- and `propose` must accept that exact file without edits. The
/// response is the documented empty one, so this also freezes how `candidates: []` is treated.
#[test]
fn emit_request_is_complete_and_accepted_by_propose() {
    let dir = collision_fixture("emit-request");
    let project = dir.to_str().expect("utf-8 path");

    // The flag is additive: the unflagged command still reports the suggestion JSON.
    let report = json(&run_ok(&["suggest", "--project", project]));
    assert_eq!(report["next"], "new_atom", "{report}");

    let first = run_ok(&["suggest", "--project", project, "--emit-request"]);
    let second = run_ok(&["suggest", "--project", project, "--emit-request"]);
    assert_eq!(
        first, second,
        "the same state must emit the same request byte-for-byte"
    );
    assert_eq!(first.lines().count(), 1, "the request is one line: {first}");
    assert!(
        !first.contains("\"next\"") && !first.contains("\"suggestions\""),
        "with --emit-request the suggestion JSON must not be printed: {first}"
    );

    let request = json(&first);
    assert_eq!(request["protocol"], "metaform.propose/1", "{request}");

    let request_id = request["request_id"]
        .as_str()
        .expect("request_id is a string");
    assert!(
        request_id.starts_with("suggest-"),
        "request_id is derived from the state: {request_id}"
    );
    assert_eq!(
        request_id.len(),
        "suggest-".len() + 16,
        "request_id carries a fixed-width state hash: {request_id}"
    );

    assert_eq!(request["objects"], 2, "{request}");
    assert_eq!(request["object_ids"], serde_json::json!([0, 1]), "{request}");
    assert_eq!(
        request["vocabulary"],
        serde_json::json!([
            {"name": "a", "ids": [0, 1]},
            {"name": "b", "ids": [0, 1]}
        ]),
        "every predicate name with its current ids, sorted by name: {request}"
    );
    assert_eq!(
        request["evidence"],
        serde_json::json!({"pos": [0], "neg": [1]}),
        "{request}"
    );
    assert_eq!(request["theta"], 3, "the recommended theta: {request}");
    assert_eq!(
        request["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "the same pair set the suggest report shows: {request}"
    );
    assert_eq!(
        request["allowed"],
        serde_json::json!({
            "ids": true,
            "define": {"ops": ["all_of", "any_of"], "operands": "vocabulary"}
        }),
        "{request}"
    );

    let mut keys: Vec<&str> = request
        .as_object()
        .expect("the request is a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "allowed",
            "colliding_pairs",
            "evidence",
            "object_ids",
            "objects",
            "protocol",
            "request_id",
            "theta",
            "vocabulary"
        ],
        "the request carries exactly the documented fields: {request}"
    );

    // `--out` writes the same one-line request and prints nothing.
    let request_path = dir.join("request.json");
    let stdout = run_ok(&[
        "suggest",
        "--project",
        project,
        "--emit-request",
        "--out",
        request_path.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(stdout, "", "--out sends the request to the file, not stdout");
    assert_eq!(
        std::fs::read_to_string(&request_path).expect("read emitted request"),
        first,
        "stdout and --out must carry the identical request"
    );

    // The emitted request is valid as-is; an empty candidate list is a successful response.
    let response_path = dir.join("response.json");
    std::fs::write(
        &response_path,
        format!(
            "{{\"protocol\":\"metaform.propose/1\",\"request_id\":\"{request_id}\",\"candidates\":[],\"refused\":null}}"
        ),
    )
    .expect("write empty response");
    let verdict = json(&run_ok(&[
        "propose",
        "--request",
        request_path.to_str().expect("utf-8 path"),
        "--response",
        response_path.to_str().expect("utf-8 path"),
    ]));
    assert_eq!(verdict["protocol"], "metaform.propose/1", "{verdict}");
    assert_eq!(verdict["request_id"], request_id, "{verdict}");
    assert_eq!(verdict["accepted"], serde_json::json!([]), "{verdict}");
    assert_eq!(verdict["rejected"], serde_json::json!([]), "{verdict}");
    assert_eq!(
        verdict["refused"],
        serde_json::Value::Null,
        "an empty candidate list is not a refusal: {verdict}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A state that does not need a new atom emits no request at all: it prints the status and `null`,
/// with no suggestion JSON, so a caller cannot mistake an empty request for a call to propose.
#[test]
fn settled_state_emits_a_null_request() {
    let undetermined = temp_dir("emit-request-undetermined");
    let project = undetermined.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    let out = run_ok(&["suggest", "--project", project, "--emit-request"]);
    assert_eq!(
        out, "{\"status\":\"UNDETERMINED\",\"request\":null}\n",
        "an open state has nothing to propose to"
    );
    assert_eq!(
        out,
        run_ok(&["suggest", "--project", project, "--emit-request"]),
        "the settled shape is deterministic"
    );

    let identified = temp_dir("emit-request-identified");
    let project = identified.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "2"]);
    run_ok(&[
        "state", "add", "--project", project, "--name", "p", "--ids", "0",
    ]);
    state_vote(project, "0", "3", "0");
    state_vote(project, "1", "0", "3");
    let out = run_ok(&["suggest", "--project", project, "--emit-request"]);
    assert_eq!(
        out, "{\"status\":\"IDENTIFIED\",\"request\":null}\n",
        "an identified state needs no proposal"
    );

    let _ = std::fs::remove_dir_all(&undetermined);
    let _ = std::fs::remove_dir_all(&identified);
}

/// The emitted request is read-only: `.metaform/state.json` keeps every byte and no file appears.
#[test]
fn emit_request_is_read_only() {
    let dir = collision_fixture("emit-request-read-only");
    let project = dir.to_str().expect("utf-8 path");
    let path = state_file(&dir);

    let state_before = std::fs::read(&path).expect("read state");
    let files_before = tree(&dir);
    let emitted = run_ok(&["suggest", "--project", project, "--emit-request"]);
    assert!(
        json(&emitted)["request_id"].is_string(),
        "the request was emitted: {emitted}"
    );
    let state_after = std::fs::read(&path).expect("read state");
    let files_after = tree(&dir);

    assert_eq!(
        state_before, state_after,
        "suggest --emit-request must leave .metaform/state.json byte-for-byte unchanged"
    );
    assert_eq!(
        files_before, files_after,
        "suggest --emit-request must not create or remove files"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
