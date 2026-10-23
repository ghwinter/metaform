//! `falsifier`: the field every proposer wrote and nothing read.
//!
//! Before this file, 596 recorded candidates carried a `falsifier`, 587 of them as prose, and no
//! line of code looked at any of them. Prose cannot be settled, so it stays prose. A pair
//! (`{"pair":[a,b]}`) can be settled, and this file pins down the two times it is settled:
//!
//!   * at admission, against the evidence the request carries -- `falsifier_bad_pair`,
//!     `falsifier_not_separating`, `falsifier_already_fired`;
//!   * later, by `check-predictions`, against labels that arrive after adoption -- `refuted`.
//!
//! The second settlement is the one that makes the field a commitment rather than a comment, so the
//! last test runs the whole path: propose, apply, learn one more label, and watch the candidate's own
//! claim fire.

use std::path::{Path, PathBuf};
use std::process::Command;

use metaform::propose::validate_response;

fn request() -> &'static str {
    r#"{"protocol":"metaform.propose/1","request_id":"r-falsifier","objects":4,
        "object_ids":[0,1,2,3],
        "vocabulary":[{"name":"a","ids":[0]}],
        "evidence":{"pos":[0],"neg":[1]},
        "allowed":{"ids":true,"define":{"ops":["all_of","any_of"],"operands":"vocabulary"}}}"#
}

fn verdict(response: &str) -> String {
    format!("{:?}", validate_response(request(), response).expect("request parses"))
}

fn candidate(body: &str) -> String {
    format!(
        r#"{{"protocol":"metaform.propose/1","request_id":"r-falsifier",
             "candidates":[{{"name":"c","ids":[0],{body},"predict":{{"id":2,"label":"pos"}}}}]}}"#
    )
}

#[test]
fn a_pair_that_is_separated_is_accepted_and_echoed_as_a_pair() {
    let out = verdict(&candidate(r#""falsifier":{"pair":[0,1]}"#));
    assert!(out.contains("falsifier_pair: Some("), "the pair must reach `apply`: {out}");
    assert!(!out.contains("rejected: [Rejected"), "the pair is well formed: {out}");

    // The echo keeps the shape it came in: a pair is answered with a pair, not flattened to prose.
    let parsed = validate_response(request(), &candidate(r#""falsifier":{"pair":[0,1]}"#))
        .expect("request parses");
    let json = metaform::propose::verdict_json(&parsed);
    assert!(json.contains(r#""falsifier":{"pair":[0,1]}"#), "echoed as a pair: {json}");
}

#[test]
fn prose_is_carried_unchanged_and_never_checked() {
    // The 587 prose falsifiers have to keep their old verdict: they are accepted, echoed verbatim,
    // and no rule looks at what they say.
    let out = verdict(&candidate(r#""falsifier":"a labelled object that contradicts me""#));
    assert!(
        out.contains("a labelled object that contradicts me"),
        "prose must survive: {out}"
    );
    assert!(!out.contains("rejected: [Rejected"), "prose cannot be refused: {out}");
}

#[test]
fn a_pair_the_extension_does_not_separate_is_refused() {
    // Neither member inside the extension: `ids: [0]` is `{0}`, so [2,3] asserts nothing.
    let out = verdict(&format!(
        r#"{{"protocol":"metaform.propose/1","request_id":"r-falsifier",
             "candidates":[{{"name":"c","ids":[0],"falsifier":{{"pair":[2,3]}},
                            "predict":{{"id":2,"label":"pos"}}}}]}}"#
    ));
    assert!(out.contains("falsifier_not_separating"), "neither is inside: {out}");

    // Both members inside: `ids: [0,1,2]` covers 0 and 1, so the same pair asserts nothing either.
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-falsifier",
            "candidates":[{"name":"c","ids":[0,1,2],"falsifier":{"pair":[0,1]},
                           "predict":{"id":3,"label":"pos"}}]}"#,
    );
    assert!(out.contains("falsifier_not_separating"), "both are inside: {out}");
}

#[test]
fn a_malformed_pair_is_refused() {
    for pair in ["[0]", "[0,0]", "[0,9]", "[0,-1]", r#"["0","1"]"#] {
        let out = verdict(&format!(
            r#"{{"protocol":"metaform.propose/1","request_id":"r-falsifier",
                 "candidates":[{{"name":"c","ids":[0],"falsifier":{{"pair":{pair}}},
                                "predict":{{"id":2,"label":"pos"}}}}]}}"#
        ));
        assert!(out.contains("falsifier_bad_pair"), "pair {pair} must be refused: {out}");
    }
    // An object without a `pair` member is the same mistake.
    let out = verdict(&candidate(r#""falsifier":{"ids":[0,1]}"#));
    assert!(out.contains("falsifier_bad_pair"), "{out}");
}

#[test]
fn a_pair_the_evidence_already_contradicts_is_refused() {
    // The extension is {1}, so the pair [0,1] implies "1 is positive" -- and the evidence says 1 is
    // negative. The candidate's own falsifier fired before it was adopted.
    let out = verdict(
        r#"{"protocol":"metaform.propose/1","request_id":"r-falsifier",
            "candidates":[{"name":"c","ids":[1],"falsifier":{"pair":[0,1]},
                           "predict":{"id":2,"label":"pos"}}]}"#,
    );
    assert!(out.contains("falsifier_already_fired"), "{out}");
}

#[test]
fn a_bare_array_is_ignored_exactly_as_before() {
    // Nine of the recorded candidates carry a bare array. It was silently dropped when the field had
    // no shape, and it is still dropped: turning it into an error now would change verdicts that
    // have already been reported.
    let out = verdict(&candidate(r#""falsifier":[0,1]"#));
    assert!(!out.contains("rejected: [Rejected"), "the candidate is still accepted: {out}");
    assert!(!out.contains("falsifier_pair: Some("), "a bare array is not a pair: {out}");
}

// ---------------------------------------------------------------------------
// The end-to-end half: the pair is recorded, and it fires when a later label contradicts it.
// ---------------------------------------------------------------------------

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-falsifier-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run_ok(args: &[&str]) -> String {
    let out = Command::new(ENGINE).args(args).output().expect("run metaform");
    assert!(
        out.status.success(),
        "`metaform {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("stdout is UTF-8")
}

/// Four objects, one predicate that merges object 0 (positive) with object 1 (negative), so the
/// state is INSUFFICIENT and `suggest --emit-request` produces a request with unmeasured objects.
fn project(tag: &str) -> (PathBuf, String) {
    let dir = temp_dir(tag);
    let p = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", p, "--n", "4"]);
    run_ok(&["state", "add", "--project", p, "--name", "a", "--ids", "0,1,2"]);
    run_ok(&["state", "vote", "--project", p, "--id", "0", "--yes", "3", "--no", "0"]);
    run_ok(&["state", "vote", "--project", p, "--id", "1", "--yes", "0", "--no", "3"]);
    (dir.clone(), p.to_string())
}

fn apply_response(dir: &Path, project: &str, falsifier: &str) -> String {
    let req = dir.join("request.json");
    run_ok(&[
        "suggest",
        "--project",
        project,
        "--emit-request",
        "--out",
        req.to_str().expect("utf-8 path"),
    ]);
    let text = std::fs::read_to_string(&req).expect("read request");
    let value: serde_json::Value = serde_json::from_str(&text).expect("request is JSON");
    let id = value["request_id"].as_str().expect("request_id").to_string();
    let response = dir.join("response.json");
    std::fs::write(
        &response,
        format!(
            r#"{{"protocol":"metaform.propose/1","request_id":"{id}","candidates":
                [{{"name":"cand","ids":[0,2],"falsifier":{falsifier},"predict":{{"id":3,"label":"pos"}}}}]}}"#
        ),
    )
    .expect("write response");
    run_ok(&[
        "apply",
        "--project",
        project,
        "--request",
        req.to_str().expect("utf-8 path"),
        "--response",
        response.to_str().expect("utf-8 path"),
    ])
}

#[test]
fn a_recorded_pair_fires_when_a_later_label_contradicts_it() {
    let (dir, project) = project("fires");

    // The extension is {0,2}, so the pair [1,2] says "2 is positive and 1 is negative". The evidence
    // already agrees about 1 and says nothing about 2, so the candidate is admissible.
    apply_response(&dir, &project, r#"{"pair":[1,2]}"#);

    let state = std::fs::read_to_string(dir.join(".metaform").join("state.json")).expect("state");
    let state: serde_json::Value = serde_json::from_str(&state).expect("state is JSON");
    let recorded = state["predicates"]
        .as_array()
        .expect("predicates")
        .iter()
        .find(|p| p["name"] == "cand")
        .expect("the adopted predicate");
    assert_eq!(
        recorded["falsifier"],
        serde_json::json!({"pair": [1, 2]}),
        "the pair must be persisted by apply"
    );
    let record =
        std::fs::read_to_string(dir.join(".metaform").join("predictions.tsv")).expect("record");
    assert!(
        record.lines().any(|l| l.ends_with("\t1,2")),
        "the pair is the eighth column of the prediction row: {record}"
    );

    // Nothing is refuted while object 2 is unmeasured.
    let before = run_ok(&["check-predictions", "--project", &project]);
    assert!(before.contains(r#""refuted":[]"#), "no label, no refutation: {before}");

    // Now the label arrives and disagrees: the pair said 2 is positive, the evidence says negative.
    run_ok(&["state", "vote", "--project", &project, "--id", "2", "--yes", "0", "--no", "3"]);
    let after = run_ok(&["check-predictions", "--project", &project]);
    assert!(
        after.contains(r#""refuted":[{"name":"cand","pair":[1,2],"fired_on":2}]"#),
        "the candidate's own falsifier must fire: {after}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
