//! stdout carries results; commentary goes to stderr.
//!
//! A pipe that reads a command's stdout must be able to parse every line. This test pins that for
//! the commands a script is most likely to call, because a stray human sentence on stdout breaks a
//! pipeline silently -- the reader gets a parse error three processes later.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

/// A scratch project that removes itself when the test ends.
///
/// The helpers name the directory after the process id, so the tests could run in parallel, but
/// nothing removed them afterwards: every run left one directory behind and the workspace filled
/// up. The guard ends that.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for Scratch {
    type Target = std::path::Path;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn project(tag: &str) -> Scratch {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("engine lives under the workspace root")
        .join("tmp");
    let dir = root.join(format!("stdout-purity-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".metaform")).expect("create scratch project");
    std::fs::write(
        dir.join(".metaform").join("state.json"),
        r#"{"n":3,"predicates":[{"name":"a","ids":[0]}],
            "votes":[{"id":0,"yes":3,"no":0},{"id":1,"yes":0,"no":3}]}"#,
    )
    .expect("write state");
    Scratch(dir)
}

fn run(project: &Path, args: &[&str]) -> (String, String, bool) {
    let out = Command::new(ENGINE)
        .args(args)
        .arg("--project")
        .arg(project)
        .output()
        .expect("run the engine");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

/// A command's stdout must be one JSON document (pretty or compact), never prose. MCP is the one
/// place where the rule is per line rather than per document, and its transcript test pins that.
fn assert_stdout_is_one_json_document(stdout: &str, command: &str) {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return;
    }
    let parsed: Result<Value, _> = serde_json::from_str(trimmed);
    assert!(
        parsed.is_ok(),
        "`{command}` wrote something other than one JSON document to stdout: {:?}",
        &trimmed[..trimmed.len().min(200)]
    );
}

#[test]
fn state_show_and_suggest_keep_stdout_parseable() {
    let project = project("show-suggest");
    for args in [vec!["state", "show"], vec!["suggest"]] {
        let (stdout, stderr, ok) = run(&project, &args);
        assert!(ok, "`{args:?}` failed; stderr: {stderr}");
        assert_stdout_is_one_json_document(&stdout, &args.join(" "));
        assert!(!stdout.contains("proposed"), "commentary leaked: {stdout}");
    }
}

#[test]
fn quiet_mode_silences_stderr_without_touching_stdout() {
    let project = project("quiet");
    let loud = run(&project, &["suggest"]);
    let quiet = Command::new(ENGINE)
        .args(["suggest", "--project"])
        .arg(&*project)
        .env("METAFFORM_QUIET", "1")
        .output()
        .expect("run the engine");
    let quiet_stdout = String::from_utf8_lossy(&quiet.stdout).into_owned();
    assert_eq!(loud.0.trim(), quiet_stdout.trim(), "quiet must not change results");
    assert_stdout_is_one_json_document(&quiet_stdout, "suggest (quiet)");
}
