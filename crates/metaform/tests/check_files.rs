//! CLI checks for the `check` subcommands that read repository files.
//!
//! These four used to be PowerShell one-liners in the claims register, so the failure they are most
//! likely to develop is the silent inverse: a rule that passes because it looked at nothing. Each
//! test therefore runs the command both ways -- once on a file that satisfies the assertion and once
//! on a file that does not -- so "ok" cannot be the only outcome it knows how to produce.

use std::path::{Path, PathBuf};
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-check-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(ENGINE)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run the binary");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().unwrap_or(-1), text)
}

#[test]
fn text_requires_one_string_and_forbids_another() {
    let dir = temp_dir("text");
    std::fs::write(dir.join("doc.md"), "installation: cargo build\n").expect("write fixture");

    let (code, out) = run(&dir, &["check", "text", "doc.md", "--has", "cargo build"]);
    assert_eq!(code, 0, "a present string must pass: {out}");
    assert!(out.contains("[check-text] ok"), "{out}");

    let (code, out) = run(&dir, &["check", "text", "doc.md", "--has", "engine"]);
    assert_eq!(code, 1, "an absent required string must fail: {out}");
    assert!(out.contains("does not contain"), "{out}");

    let (code, out) = run(&dir, &["check", "text", "doc.md", "--lacks", "cargo build"]);
    assert_eq!(code, 1, "a present forbidden string must fail: {out}");

    let (code, out) = run(&dir, &["check", "text", "doc.md", "--lacks", "2055-11-25"]);
    assert_eq!(code, 0, "an absent forbidden string must pass: {out}");

    // Two files, one of which is missing the string: the assertion is about every path named.
    std::fs::write(dir.join("other.md"), "nothing here\n").expect("write fixture");
    let (code, out) = run(&dir, &["check", "text", "doc.md", "other.md", "--has", "cargo build"]);
    assert_eq!(code, 1, "the second file must be checked too: {out}");
    assert!(out.contains("other.md"), "{out}");
}

#[test]
fn count_measures_lines_beginning_with_a_marker() {
    let dir = temp_dir("count");
    std::fs::write(dir.join("a.md"), "# one\n## two\n## three\ntext\n").expect("write fixture");
    std::fs::write(dir.join("b.md"), "# one\n## two\n").expect("write fixture");

    let (code, out) = run(&dir, &["check", "count", "a.md", "--lines-begin", "## ", "--equals", "2"]);
    assert_eq!(code, 0, "{out}");

    let (code, out) = run(&dir, &["check", "count", "a.md", "--contains", "##", "--equals", "2"]);
    assert_eq!(code, 0, "two lines contain the marker even when it is not first: {out}");

    let (code, out) = run(&dir, &["check", "count", "a.md", "--lines-begin", "## ", "--equals", "3"]);
    assert_eq!(code, 1, "a wrong count must fail: {out}");

    // Two files that agree.
    let (code, out) = run(&dir, &["check", "count", "a.md", "--lines-begin", "#", "--same", "a.md"]);
    assert_eq!(code, 0, "a file always agrees with itself: {out}");

    let (code, out) = run(&dir, &["check", "count", "a.md", "--lines-begin", "## ", "--same", "b.md"]);
    assert_eq!(code, 1, "2 != 1 must fail: {out}");

    // Different markers on the two sides, which is how the bilingual documents are compared.
    std::fs::write(dir.join("d.md"), "?? one\n?? two\n?? three\n").expect("write fixture");
    let (code, out) = run(
        &dir,
        &["check", "count", "a.md", "--lines-begin", "#", "--same", "d.md", "--pattern2", "??"],
    );
    assert_eq!(code, 0, "3 markers of one shape, 3 of another: {out}");
}

#[test]
fn count_reads_the_number_after_a_marker() {
    let dir = temp_dir("number");
    std::fs::write(dir.join("gate.ps1"), "[int]$Expected = 175,\n").expect("write fixture");
    std::fs::write(dir.join("doc.md"), "the pass count must == 175 total\n").expect("write fixture");

    let (code, out) = run(&dir, &["check", "count", "doc.md", "--number-after", "pass count must == "]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("175"), "{out}");

    let (code, out) = run(
        &dir,
        &[
            "check", "count", "doc.md", "--number-after", "pass count must == ", "--same", "gate.ps1",
            "--pattern2", "Expected = ",
        ],
    );
    assert_eq!(code, 0, "both files state 175: {out}");

    std::fs::write(dir.join("stale.md"), "the pass count must == 174 total\n").expect("write fixture");
    let (code, out) = run(
        &dir,
        &[
            "check", "count", "stale.md", "--number-after", "pass count must == ", "--same", "gate.ps1",
            "--pattern2", "Expected = ",
        ],
    );
    assert_eq!(code, 1, "174 != 175 must fail: {out}");

    let (code, out) = run(&dir, &["check", "count", "doc.md", "--number-after", "absent marker"]);
    assert_eq!(code, 1, "a missing marker must fail rather than count zero: {out}");
}

#[test]
fn fresh_compares_an_artefact_with_its_source() {
    let dir = temp_dir("fresh");
    std::fs::write(dir.join("main.tex"), "source\n").expect("write source");
    std::fs::write(dir.join("main.pdf"), "artefact\n").expect("write artefact");

    let (code, out) = run(&dir, &["check", "fresh", "main.pdf", "--than", "main.tex"]);
    assert_eq!(code, 0, "the artefact was written second: {out}");
    assert!(out.contains("not older"), "{out}");

    // Rewriting the source makes the artefact stale again.
    std::fs::write(dir.join("main.tex"), "source, edited\n").expect("rewrite source");
    let (code, out) = run(&dir, &["check", "fresh", "main.pdf", "--than", "main.tex"]);
    assert_eq!(code, 1, "a stale artefact must fail: {out}");

    let (code, out) = run(&dir, &["check", "fresh", "missing.pdf", "--than", "main.tex"]);
    assert_eq!(code, 1, "a missing artefact must fail rather than pass: {out}");
    assert!(out.contains("cannot stat"), "{out}");
}

#[test]
fn absent_fails_when_the_path_is_there() {
    let dir = temp_dir("absent");
    std::fs::create_dir_all(dir.join("target")).expect("create stale directory");

    let (code, out) = run(&dir, &["check", "absent", "target"]);
    assert_eq!(code, 1, "a stale build directory must fail: {out}");
    assert!(out.contains("present, expected absent"), "{out}");

    let (code, out) = run(&dir, &["check", "absent", "crates/metaform/target"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn an_unknown_option_is_refused_rather_than_read_as_a_path() {
    let dir = temp_dir("typo");
    std::fs::write(dir.join("doc.md"), "text\n").expect("write fixture");
    let (code, out) = run(&dir, &["check", "text", "doc.md", "--haz", "text"]);
    assert_eq!(code, 1, "a typo must fail: {out}");
    assert!(out.contains("unknown option"), "{out}");
}
