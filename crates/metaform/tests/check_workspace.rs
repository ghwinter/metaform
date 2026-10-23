//! The two checks that used to look at one crate and now look at the workspace.
//!
//! `prints` and `layers` were written when this repository was a single crate. Nine members later
//! they were still reading `crates/metaform/src` alone, so every other crate could print freely and
//! no dependency edge between crates was ever read. The failure mode is the quiet one -- the
//! command still exits 0 -- so the tests assert on *what the output mentions*, not just on the exit
//! code: a scan that regresses to one directory would still pass a bare `assert_eq!(code, 0)`.

use std::path::{Path, PathBuf};
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-check-workspace-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(ENGINE).args(args).current_dir(dir).output().expect("run the binary");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().unwrap_or(-1), text)
}

#[test]
fn prints_counts_sites_outside_the_engine_crate() {
    // Run from a temporary directory: the scan is about the workspace this binary was built in, not
    // about wherever the caller happens to stand.
    let dir = temp_dir("prints");
    let (code, out) = run(&dir, &["check", "prints"]);
    assert_eq!(code, 0, "the tree prints no kernel module: {out}");
    assert!(
        out.contains("components/bin/"),
        "a print site in a member other than metaform must appear by name: {out}"
    );
    assert!(out.contains("metaform/main.rs="), "the engine's own sites are still counted: {out}");
}

#[test]
fn layers_reads_the_workspace_dependency_graph() {
    let dir = temp_dir("layers");
    let (code, out) = run(&dir, &["check", "layers"]);
    assert_eq!(code, 0, "every member depends downward: {out}");
    assert!(out.contains("workspace dependency edge(s)"), "the crate graph must be measured: {out}");
    // The count is not asserted: a tenth member should be a passing change here, and a red one in
    // `every_workspace_member_has_a_layer` only if its manifest forgot the table.
    assert!(out.contains("file(s) classified") || out.contains("files classified"), "{out}");
}
