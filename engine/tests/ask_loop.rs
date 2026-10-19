//! End-to-end check of the `ask` loop, CLI only: `suggest` names an object, a person answers it, the
//! answer is recorded, and `state need` / `suggest` see the new state.
//!
//! The fixture is the one `engine/tests/suggest.rs` uses for `unprobed_member`: one atom covers `{0,1,2}`,
//! object 0 is a strong positive, object 1 a strong negative, and object 2 has never been asked. The
//! class is mixed, so the pair (0,1) collides and `ask` names the unprobed member 2.
//!
//! What the two tests measure (they were run, not assumed):
//!
//! - A normal answer is recorded and changes the state: `state.json` gains the vote, `ledger.tsv`
//!   gains row `L1` (`by=ask`, `answer=true`), `unverified` drops from `[2]` to `[]`, and the next
//!   `suggest` recommends `new_atom` -- every member of the class is now probed, so no question is
//!   left. (Under the old count-based rule this state reported `ask`, anchor 0,
//!   `conflicting_labels`, forever; that fixed point is what the dispatch fix removed.)
//! - The original colliding pair (0,1) does **not** disappear. A vote only adds evidence, and asking
//!   cannot remove a collision (Theorem 5; `suggest.rs`: "Asking appends evidence and cannot remove
//!   a colliding pair"). Object 2 is a third member of the colliding class, so its answer adds a
//!   pair -- `(2,1)` when it answers positive, `(0,2)` when it answers negative -- instead of
//!   retracting either label of 0 or 1. The test asserts the measured pair set and says so, rather
//!   than asserting the pair is gone.
//! - The inconsistent answer is not a separate branch. It is recorded, the class is treated as
//!   conflicting and fully probed, the status stays `INSUFFICIENT`, and `suggest` recommends
//!   `new_atom`: more questions cannot repair the class.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-ask-loop-{}-{tag}", std::process::id()));
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

/// One atom covers `{0,1,2}`; 0 is a strong positive, 1 a strong negative, 2 was never asked. The
/// class is mixed, so the pair (0,1) collides and `ask` names 2 as the unprobed member.
fn inherited_fixture(tag: &str) -> PathBuf {
    let dir = temp_dir(tag);
    let project = dir.to_str().expect("utf-8 path");
    run_ok(&["state", "init", "--project", project, "--n", "3"]);
    run_ok(&[
        "state",
        "add",
        "--project",
        project,
        "--name",
        "a",
        "--ids",
        "0,1,2",
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

fn need(project: &str) -> serde_json::Value {
    json(&run_ok(&["state", "need", "--project", project]))
}

/// Run `suggest` and assert it left `.metaform/state.json` byte-for-byte unchanged.
fn suggest_read_only(dir: &Path) -> serde_json::Value {
    let project = dir.to_str().expect("utf-8 path");
    let before = state_bytes(dir);
    let report = json(&run_ok(&["suggest", "--project", project]));
    let after = state_bytes(dir);
    assert_eq!(
        before, after,
        "suggest must leave .metaform/state.json byte-for-byte unchanged"
    );
    report
}

/// Ledger rows, including the column-name row: row 0 is the columns, row 1 the first entry.
fn ledger_rows(project: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(project.join(".metaform").join("ledger.tsv"))
        .expect("read the ledger");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.split('\t').map(str::to_string).collect())
        .collect()
}

fn ledger_columns() -> Vec<String> {
    [
        "id",
        "case",
        "status",
        "by",
        "round",
        "reliability",
        "answer",
        "reason",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[test]
fn consistent_answer_is_recorded_and_ends_the_ask_loop() {
    let dir = inherited_fixture("consistent");
    let project = dir.to_str().expect("utf-8 path");

    // Step 1: the state is insufficient, and `suggest` names the never-asked member.
    let before = need(project);
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(before["action"], "PROPOSE", "{before}");
    assert_eq!(before["classes"], 1, "{before}");
    assert_eq!(
        before["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{before}"
    );
    assert_eq!(
        before["unverified"],
        serde_json::json!([2]),
        "object 2 was never asked directly: {before}"
    );

    assert!(
        !dir.join(".metaform").join("ledger.tsv").exists(),
        "the fixture has no ledger before the recorded vote"
    );
    let report = suggest_read_only(&dir);
    assert_eq!(report["status"], "INSUFFICIENT", "{report}");
    assert_eq!(
        report["reason"], "boolean_recombination_cannot_separate",
        "{report}"
    );
    assert_eq!(report["suggestions"], serde_json::json!([]), "{report}");
    assert_eq!(
        report["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{report}"
    );
    assert_eq!(
        report["next"], "ask",
        "the class still has an unasked member (object 2): {report}"
    );
    assert_eq!(report["ask"]["why"], "unprobed_member", "{report}");
    assert_eq!(report["ask"]["questions"], 1, "{report}");
    assert_eq!(
        report["ask"]["objects"],
        serde_json::json!([2]),
        "the ask must name the never-asked member: {report}"
    );

    // Step 2: answer it the way a person would.
    //
    // The class is mixed, so the collision alone does not name one side: 0 is positive and 1 is
    // negative in the same class. The side this test calls consistent is the positive one. The tool
    // itself breaks every class tie by the lowest member (`free_reps`, and `ask_plan`'s
    // `anchor = trailing_zeros`), object 0 is that member and a strong positive, and `why:
    // unprobed_member` claims object 2's label is inherited from the probed class. So object 2
    // answers the way its class representative does: yes.
    let bytes_before_vote = state_bytes(&dir);
    let vote = json(&run_ok(&[
        "state",
        "vote",
        "--project",
        project,
        "--id",
        "2",
        "--yes",
        "3",
        "--no",
        "0",
        "--record",
        "--by",
        "ask",
        "--reason",
        "suggest named object 2",
    ]));
    assert_eq!(vote["ok"], true, "{vote}");
    assert_eq!(vote["id"], 2, "{vote}");
    assert_eq!(
        vote["ledger"], "L1",
        "the vote must land in the ledger: {vote}"
    );

    // The vote is the write: `suggest` above did not touch the state, the vote does.
    assert_ne!(
        state_bytes(&dir),
        bytes_before_vote,
        "the recorded vote must change .metaform/state.json"
    );
    let saved = json(&std::fs::read_to_string(state_file(&dir)).expect("read state"));
    assert_eq!(
        saved["votes"],
        serde_json::json!([
            {"id": 0, "yes": 3, "no": 0},
            {"id": 1, "yes": 0, "no": 3},
            {"id": 2, "yes": 3, "no": 0}
        ]),
        "the vote must be persisted in .metaform/state.json: {saved}"
    );

    let rows = ledger_rows(&dir);
    assert_eq!(rows.len(), 2, "column row plus one evidence row: {rows:?}");
    assert_eq!(rows[0], ledger_columns(), "ledger header changed: {rows:?}");
    assert_eq!(
        rows[1],
        vec![
            "L1",
            "2",
            "active",
            "ask",
            "0",
            "1.00",
            "true",
            "suggest named object 2"
        ],
        "the answer is recorded with its direction and reason: {rows:?}"
    );

    // Step 3: `need` re-evaluates. Measured: the pair (0,1) is NOT gone.
    //
    // The instruction for this test asked for "the specific colliding pair is gone". It does not
    // happen, and the engine's own theorem says why: a vote only adds evidence, asking cannot repair
    // a collision (Theorem 5). Object 2 is a third member of the colliding class, so its positive
    // answer adds the pair (2,1); it cannot retract 0's positive or 1's negative label. The assertion
    // below therefore records the measured pair set, and states the surviving pair explicitly.
    let after = need(project);
    assert_eq!(after["status"], "INSUFFICIENT", "{after}");
    assert_eq!(
        after["colliding_pairs"],
        serde_json::json!([[0, 1], [2, 1]]),
        "{after}"
    );
    let pairs = after["colliding_pairs"]
        .as_array()
        .expect("colliding_pairs is an array");
    assert!(
        pairs.contains(&serde_json::json!([0, 1])),
        "measured, not assumed: answering object 2 leaves the original pair (0,1) colliding: {after}"
    );
    assert_eq!(
        after["unverified"],
        serde_json::json!([]),
        "the named object was asked directly: {after}"
    );
    assert_eq!(after["pending"], serde_json::json!([]), "{after}");

    // Step 4: `suggest` again. The class is now fully probed and its labels disagree, so no
    // question is left: the recommendation changes from `ask` to `new_atom` and the `ask` field is
    // gone. This is the fixed point the count-based rule used to report (`ask`, anchor 0,
    // `conflicting_labels`, forever) -- the measured pair set changed the recommendation, so it is
    // no longer byte-identical to the pre-answer report.
    let report_after = suggest_read_only(&dir);
    assert_eq!(report_after["status"], "INSUFFICIENT", "{report_after}");
    assert_eq!(report_after["colliding"], 2, "{report_after}");
    assert_eq!(
        report_after["suggestions"],
        serde_json::json!([]),
        "{report_after}"
    );
    assert_eq!(
        report_after["next"], "new_atom",
        "every member was asked, so no question can repair the class: {report_after}"
    );
    assert!(
        report_after.get("ask").is_none(),
        "new_atom must omit the ask field entirely: {report_after}"
    );
    assert_ne!(
        report_after["next"], report["next"],
        "the answer changes the recommendation instead of repeating it: {report_after}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inconsistent_answer_is_recorded_and_recommends_new_atom() {
    let dir = inherited_fixture("inconsistent");
    let project = dir.to_str().expect("utf-8 path");

    // Same start: the question is object 2, the unprobed member.
    let before = need(project);
    assert_eq!(before["status"], "INSUFFICIENT", "{before}");
    assert_eq!(
        before["colliding_pairs"],
        serde_json::json!([[0, 1]]),
        "{before}"
    );
    let report = suggest_read_only(&dir);
    assert_eq!(report["next"], "ask", "{report}");
    assert_eq!(report["ask"]["objects"], serde_json::json!([2]), "{report}");
    assert_eq!(report["ask"]["why"], "unprobed_member", "{report}");

    // The inconsistent answer: the opposite of the inherited positive label. Object 2 says it is
    // not a member, so it contradicts the label `unprobed_member` says it inherits from the class.
    let vote = json(&run_ok(&[
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
        "--record",
        "--by",
        "ask",
        "--reason",
        "inconsistent answer: object 2 is not in the set",
    ]));
    assert_eq!(vote["ok"], true, "{vote}");
    assert_eq!(vote["ledger"], "L1", "{vote}");

    // The ledger keeps the direction of the answer, so the contradiction is evidence, not a silent
    // overwrite.
    let rows = ledger_rows(&dir);
    assert_eq!(rows.len(), 2, "column row plus one evidence row: {rows:?}");
    assert_eq!(
        rows[1],
        vec![
            "L1",
            "2",
            "active",
            "ask",
            "0",
            "1.00",
            "false",
            "inconsistent answer: object 2 is not in the set"
        ],
        "{rows:?}"
    );

    // What the tool does, measured rather than assumed: it does not settle silently. The class is
    // now fully probed and the labels still disagree, so `state need` stays INSUFFICIENT with the
    // second pair the negative answer created: (0,2).
    let after = need(project);
    assert_eq!(
        after["status"], "INSUFFICIENT",
        "an inconsistent answer must not become a silent settlement: {after}"
    );
    assert_eq!(
        after["colliding_pairs"],
        serde_json::json!([[0, 1], [0, 2]]),
        "{after}"
    );
    assert_eq!(after["action"], "PROPOSE", "{after}");
    assert_eq!(after["unverified"], serde_json::json!([]), "{after}");

    // `suggest` recommends `new_atom`: every member was asked, so the conflict is a fixed point for
    // questions and no `ask` field is emitted. (Changed under the new dispatch rule: this used to
    // stay on `ask`, naming the anchor with `conflicting_labels`, forever.) There is no separate
    // branch for an inconsistent answer: it is the same shape as the consistent case, with the pair
    // on the other side.
    let report_after = suggest_read_only(&dir);
    assert_eq!(report_after["status"], "INSUFFICIENT", "{report_after}");
    assert_eq!(report_after["colliding"], 2, "{report_after}");
    assert_eq!(
        report_after["suggestions"],
        serde_json::json!([]),
        "{report_after}"
    );
    assert_eq!(
        report_after["next"], "new_atom",
        "the engine asks no question it cannot answer, but does not settle either: {report_after}"
    );
    assert!(
        report_after.get("ask").is_none(),
        "a fully probed conflict has no question left: {report_after}"
    );
    let pairs = report_after["colliding_pairs"]
        .as_array()
        .expect("colliding_pairs is an array");
    assert!(
        pairs.contains(&serde_json::json!([0, 1])),
        "the inconsistent answer also leaves the original pair (0,1) colliding: {report_after}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
