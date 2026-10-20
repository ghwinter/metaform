//! CLI checks for the `explore --emit-questions` -> `answer` -> `explore` loop.
//!
//! The acceptance scenario is four objects, `a = {2}`, `b = {3}`, and only object 0 labelled. The
//! evidence decides neither direction, so the first run asks `a -> b` and `b -> a`. A yes answer and
//! a no answer with object 3 as counterexample are recorded in `.metaform/laws.tsv`; the second run
//! starts from those laws and has no open questions left. The tests also freeze the three answer
//! forms, the validation that must leave every file untouched, and the laws-file format.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");
const HEADER: &str = "source\tkind\tpremise\tconclusion\tnote";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-answers-{}-{tag}", std::process::id()));
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

/// `a = {2}`, `b = {3}`, object 0 strongly positive. Objects 1, 2 and 3 are not evidence, so the
/// evidence leaves both implications open: `extent(a) = {2}` and `extent(b) = {3}`.
fn acceptance_project(tag: &str) -> PathBuf {
    project(
        tag,
        json!({
            "n": 4,
            "predicates": [
                {"name": "a", "ids": [2]},
                {"name": "b", "ids": [3]}
            ],
            "votes": [
                {"id": 0, "yes": 3, "no": 0}
            ]
        }),
    )
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
    serde_json::from_str(text.trim()).expect("one JSON value")
}

fn state_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("state.json")
}

fn laws_file(project: &Path) -> PathBuf {
    project.join(".metaform").join("laws.tsv")
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).expect("write JSON");
}

fn emit_questions(project: &Path, questions: &Path, report: &Path) -> (Value, Value) {
    let out = run_ok(&[
        "explore",
        "--project",
        project.to_str().unwrap(),
        "--emit-questions",
        questions.to_str().unwrap(),
        "--out",
        report.to_str().unwrap(),
    ]);
    assert!(
        out.is_empty(),
        "--emit-questions must suppress the report line on stdout, got {out:?}"
    );
    (
        json(&std::fs::read_to_string(questions).expect("read questions")),
        json(&std::fs::read_to_string(report).expect("read report")),
    )
}

fn premises(doc: &Value) -> HashSet<String> {
    doc["questions"]
        .as_array()
        .expect("questions array")
        .iter()
        .map(|q| {
            q["premise"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect()
}

#[test]
fn two_expert_answers_close_the_loop() {
    let dir = acceptance_project("loop");
    let state_before = std::fs::read(state_file(&dir)).expect("read state");
    let qpath = dir.join("questions.json");
    let report_path = dir.join("report.json");

    let (first, first_report) = emit_questions(&dir, &qpath, &report_path);
    assert_eq!(first["status"], "UNDETERMINED");
    let questions = first["questions"].as_array().expect("questions");
    assert_eq!(questions.len(), 2, "{first}");
    assert_eq!(questions[0]["id"], 0);
    assert_eq!(questions[0]["premise"], json!(["a"]));
    assert_eq!(questions[0]["conclusion"], json!(["b"]));
    assert_eq!(
        questions[0]["text"],
        "Does every object with a also have b?"
    );
    assert_eq!(questions[1]["id"], 1);
    assert_eq!(questions[1]["premise"], json!(["b"]));
    assert_eq!(questions[1]["conclusion"], json!(["a"]));
    assert_eq!(
        questions[1]["text"],
        "Does every object with b also have a?"
    );
    assert_eq!(first_report["questions"], 2);
    assert_eq!(first_report["open"].as_array().unwrap().len(), 2);
    assert_eq!(
        std::fs::read(state_file(&dir)).expect("read state"),
        state_before,
        "explore must not write the state"
    );

    let apath = dir.join("answers.json");
    write_json(
        &apath,
        &json!({"answers": [
            {"id": 0, "answer": "yes"},
            {"id": 1, "answer": "no", "counterexample": {"id": 3, "lacks": ["a"]}}
        ]}),
    );
    let summary = json(&run_ok(&[
        "answer",
        "--project",
        dir.to_str().unwrap(),
        "--questions",
        qpath.to_str().unwrap(),
        "--answers",
        apath.to_str().unwrap(),
    ]));
    assert_eq!(summary["answers"], 2);
    assert_eq!(summary["laws"], 1);
    assert_eq!(summary["refutations"], 1);
    assert_eq!(summary["duplicates"], 0);
    assert_eq!(summary["defined"], json!([]));
    assert_eq!(
        std::fs::read(state_file(&dir)).expect("read state"),
        state_before,
        "yes/no answers must not write the state"
    );

    let text = std::fs::read_to_string(laws_file(&dir)).expect("read laws");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], HEADER);
    assert_eq!(lines.len(), 3, "one header, one law and one refutation: {text:?}");
    let rows: Vec<Vec<&str>> = lines[1..].iter().map(|line| line.split('\t').collect()).collect();
    for row in &rows {
        assert_eq!(row.len(), 5, "five columns: {row:?}");
        assert_eq!(row[0], "expert", "every entry records its source: {row:?}");
    }
    assert_eq!((rows[0][1], rows[0][2], rows[0][3]), ("law", "a", "b"));
    assert_eq!(
        (rows[1][1], rows[1][2], rows[1][3]),
        ("refutation", "b", "a"),
        "a no answer is a refutation, never a law"
    );
    assert!(
        rows[1][4].contains('3'),
        "the refutation note names the object: {:?}",
        rows[1][4]
    );

    // Replaying the same answers must not duplicate any line.
    let laws_before = std::fs::read(laws_file(&dir)).expect("read laws");
    let replay = json(&run_ok(&[
        "answer",
        "--project",
        dir.to_str().unwrap(),
        "--questions",
        qpath.to_str().unwrap(),
        "--answers",
        apath.to_str().unwrap(),
    ]));
    assert_eq!(replay["laws"], 0);
    assert_eq!(replay["refutations"], 0);
    assert_eq!(replay["duplicates"], 2);
    assert_eq!(
        std::fs::read(laws_file(&dir)).expect("read laws"),
        laws_before,
        "a repeated answer is a no-op"
    );

    // The next explore starts from the law and the refutation: no open questions, and the refuted
    // direction is never emitted as an implication.
    let qpath2 = dir.join("questions2.json");
    let report2_path = dir.join("report2.json");
    let (second, second_report) = emit_questions(&dir, &qpath2, &report2_path);
    assert_eq!(second_report["questions"], 0, "{second_report}");
    assert_eq!(second_report["open"], json!([]));
    assert!(second_report.get("stopped").is_none());
    assert!(
        second_report["implications"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| !(i["premise"] == json!(["a"]) && i["conclusion"] == json!(["b"]))),
        "the refuted a -> b must not come back as an implication: {second_report}"
    );
    assert!(
        second_report["refuted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["premise"] == json!(["b"])
                && r["conclusion"] == json!(["a"])
                && r["by"] == "expert"),
        "the recorded refutation is reported as decided: {second_report}"
    );
    assert_eq!(second["status"], "UNDETERMINED");
    assert_eq!(second["questions"], json!([]), "the loop converged: {second}");
    assert!(
        premises(&first).is_disjoint(&premises(&second)),
        "no premise may be asked twice across runs"
    );
    assert_eq!(
        std::fs::read(state_file(&dir)).expect("read state"),
        state_before,
        "the whole loop leaves the state untouched"
    );
}

#[test]
fn define_answer_adopts_the_predicate_and_records_its_law() {
    let dir = acceptance_project("define");
    let state_before = std::fs::read(state_file(&dir)).expect("read state");
    let qpath = dir.join("questions.json");
    let report_path = dir.join("report.json");
    let (first, _) = emit_questions(&dir, &qpath, &report_path);
    let yes_question = first["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["premise"] == json!(["a"]))
        .expect("a -> b question")
        .clone();

    let apath = dir.join("answers.json");
    write_json(
        &apath,
        &json!({"answers": [{
            "id": yes_question["id"],
            "answer": "define",
            "name": "c",
            "op": "all_of",
            "operands": ["a"]
        }]}),
    );
    let summary = json(&run_ok(&[
        "answer",
        "--project",
        dir.to_str().unwrap(),
        "--questions",
        qpath.to_str().unwrap(),
        "--answers",
        apath.to_str().unwrap(),
    ]));
    assert_eq!(summary["defined"], json!(["c"]));
    assert_eq!(summary["laws"], 1);
    assert_eq!(summary["refutations"], 0);

    let state: Value =
        serde_json::from_str(&std::fs::read_to_string(state_file(&dir)).expect("read state"))
            .expect("state JSON");
    let c = state["predicates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "c")
        .expect("c was adopted");
    assert_eq!(c["ids"], json!([2]), "c = a = {{2}}");
    assert_eq!(c["definition"]["op"], "all_of");
    assert_eq!(c["definition"]["operands"], json!(["a"]));

    let text = std::fs::read_to_string(laws_file(&dir)).expect("read laws");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], HEADER);
    assert_eq!(lines.len(), 2, "{text:?}");
    let row: Vec<&str> = lines[1].split('\t').collect();
    assert_eq!(row[0], "define");
    assert_eq!(row[1], "law");
    assert_eq!((row[2], row[3]), ("a", "b"));
    assert!(row[4].contains('c'), "the note names the definition: {row:?}");

    // A second define of the same name is an error and changes nothing.
    let state_after = std::fs::read(state_file(&dir)).expect("read state");
    let laws_after = std::fs::read(laws_file(&dir)).expect("read laws");
    let refused = run(&[
        "answer",
        "--project",
        dir.to_str().unwrap(),
        "--questions",
        qpath.to_str().unwrap(),
        "--answers",
        apath.to_str().unwrap(),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("already exists"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert_eq!(std::fs::read(state_file(&dir)).expect("read state"), state_after);
    assert_eq!(std::fs::read(laws_file(&dir)).expect("read laws"), laws_after);
    assert_ne!(state_after, state_before, "the define answer did write the state");
}

#[test]
fn invalid_answers_exit_2_and_write_nothing() {
    let dir = acceptance_project("invalid");
    let qpath = dir.join("questions.json");
    let report_path = dir.join("report.json");
    emit_questions(&dir, &qpath, &report_path);
    let state_before = std::fs::read(state_file(&dir)).expect("read state");

    let cases = [
        // Unknown question id.
        json!({"answers": [{"id": 99, "answer": "yes"}]}),
        // Object 3 does not have premise a, so it cannot refute a -> b.
        json!({"answers": [{"id": 0, "answer": "no", "counterexample": {"id": 3, "lacks": ["b"]}}]}),
        // Object 2 has a, so it cannot be said to lack a.
        json!({"answers": [{"id": 0, "answer": "no", "counterexample": {"id": 2, "lacks": ["a"]}}]}),
        // A conclusion attribute that is not on the question.
        json!({"answers": [{"id": 0, "answer": "no", "counterexample": {"id": 2, "lacks": ["ghost"]}}]}),
        // The definition name already exists.
        json!({"answers": [{"id": 0, "answer": "define", "name": "a", "op": "all_of", "operands": ["b"]}]}),
        // The definition operand does not exist.
        json!({"answers": [{"id": 0, "answer": "define", "name": "c", "op": "all_of", "operands": ["ghost"]}]}),
        // Malformed: no answer field.
        json!({"answers": [{"id": 0}]}),
        // One valid answer and one invalid answer: the whole batch is rejected.
        json!({"answers": [
            {"id": 0, "answer": "yes"},
            {"id": 1, "answer": "no", "counterexample": {"id": 2, "lacks": ["a"]}}
        ]}),
    ];

    for (i, answers) in cases.iter().enumerate() {
        let apath = dir.join(format!("answers-{i}.json"));
        write_json(&apath, answers);
        let out = run(&[
            "answer",
            "--project",
            dir.to_str().unwrap(),
            "--questions",
            qpath.to_str().unwrap(),
            "--answers",
            apath.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(2), "case {i} must exit 2: {answers}");
        assert!(
            !laws_file(&dir).exists(),
            "case {i} must not create the laws file: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            std::fs::read(state_file(&dir)).expect("read state"),
            state_before,
            "case {i} must not write the state"
        );
    }
}

#[test]
fn recorded_refutation_decides_its_premise() {
    let dir = acceptance_project("seeded-refutation");
    // a = {2}, b = {3}: object 2 has a and lacks b, so a -> b is genuinely refuted.
    std::fs::write(
        laws_file(&dir),
        format!("{HEADER}\nexpert\trefutation\ta\tb\trefuted by object 2: lacks b\n"),
    )
    .unwrap();

    let qpath = dir.join("questions.json");
    let report_path = dir.join("report.json");
    let (questions, report) = emit_questions(&dir, &qpath, &report_path);

    // (a) No question with premise a, either in the report or in the emitted questions.
    assert!(
        report["open"]
            .as_array()
            .unwrap()
            .iter()
            .all(|q| q["premise"] != json!(["a"])),
        "the refuted premise must not be asked: {report}"
    );
    assert!(
        questions["questions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|q| q["premise"] != json!(["a"])),
        "the refuted premise must not be emitted: {questions}"
    );

    // (b) No implication with premise a and conclusion b.
    assert!(
        report["implications"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| !(i["premise"] == json!(["a"]) && i["conclusion"] == json!(["b"]))),
        "the refuted implication must not be derived: {report}"
    );

    // (c) The refutation is listed as decided, with its source and counterexample object.
    assert!(
        report["refuted"].as_array().unwrap().iter().any(|r| {
            r["premise"] == json!(["a"])
                && r["conclusion"] == json!(["b"])
                && r["counterexample"] == 2
                && r["by"] == "expert"
        }),
        "the recorded refutation must appear in refuted: {report}"
    );

    // A refutation decides only its own premise; the other direction is still asked.
    assert!(
        report["open"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q["premise"] == json!(["b"])),
        "only premise a is decided: {report}"
    );
}

#[test]
fn laws_file_seeds_the_theory_and_unknown_names_fail() {
    let dir = acceptance_project("laws");
    // No laws file yet: both directions are open.
    let qpath = dir.join("q-none.json");
    let report_path = dir.join("r-none.json");
    let (none, _) = emit_questions(&dir, &qpath, &report_path);
    assert_eq!(none["questions"].as_array().unwrap().len(), 2);

    // A hand-written default laws file decides a -> b.
    std::fs::write(
        laws_file(&dir),
        format!("{HEADER}\nexpert\tlaw\ta\tb\tanswer from the expert\n"),
    )
    .unwrap();
    let qpath = dir.join("q-default.json");
    let report_path = dir.join("r-default.json");
    let (seeded, report) = emit_questions(&dir, &qpath, &report_path);
    assert_eq!(report["questions"], 1, "{report}");
    let asked: Vec<Vec<String>> = seeded["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            q["premise"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_str().unwrap().to_string())
                .collect()
        })
        .collect();
    assert_eq!(asked, vec![vec!["b".to_string()]]);

    // An explicit --laws path overrides the default.
    let custom = dir.join("custom.tsv");
    std::fs::write(
        &custom,
        format!("{HEADER}\ndefine\tlaw\tb\ta\tb is definitional\n"),
    )
    .unwrap();
    let qpath = dir.join("q-custom.json");
    let report_path = dir.join("r-custom.json");
    let out = run_ok(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--laws",
        custom.to_str().unwrap(),
        "--emit-questions",
        qpath.to_str().unwrap(),
        "--out",
        report_path.to_str().unwrap(),
    ]);
    assert!(out.is_empty());
    let report = json(&std::fs::read_to_string(&report_path).unwrap());
    assert_eq!(report["questions"], 1, "{report}");
    let asked = json(&std::fs::read_to_string(&qpath).unwrap())["questions"][0]["premise"].clone();
    assert_eq!(asked, json!(["a"]));

    // `answer --laws` writes the same custom theory file that `explore --laws` reads.
    let custom_laws = dir.join("custom-answered.tsv");
    let custom_answers = dir.join("custom-answers.json");
    write_json(&custom_answers, &json!({"answers": [{"id": 0, "answer": "yes"}]}));
    run_ok(&[
        "answer",
        "--project",
        dir.to_str().unwrap(),
        "--questions",
        qpath.to_str().unwrap(),
        "--answers",
        custom_answers.to_str().unwrap(),
        "--laws",
        custom_laws.to_str().unwrap(),
    ]);
    let written = std::fs::read_to_string(&custom_laws).expect("read custom laws");
    assert!(written.starts_with(HEADER), "{written:?}");
    assert!(
        written.lines().any(|line| line == "expert\tlaw\ta\tb\tanswered yes to question 0"),
        "{written:?}"
    );

    // A name the context does not have is an error naming that name, not a silently dropped law.
    let bad = dir.join("bad.tsv");
    std::fs::write(&bad, format!("{HEADER}\nexpert\tlaw\ta\tghost\ttypo\n")).unwrap();
    let refused = run(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--laws",
        bad.to_str().unwrap(),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("ghost"),
        "the error must name the missing predicate: {}",
        String::from_utf8_lossy(&refused.stderr)
    );

    // An absent explicit path is the empty theory, exactly like the absent default.
    let missing = dir.join("does-not-exist.tsv");
    let report = json(&run_ok(&[
        "explore",
        "--project",
        dir.to_str().unwrap(),
        "--laws",
        missing.to_str().unwrap(),
    ]));
    assert_eq!(report["questions"], 2);
}
