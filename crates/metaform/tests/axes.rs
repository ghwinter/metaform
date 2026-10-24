//! The axes command: what the current vocabulary leaves open, and what it leaves closed.

use std::path::PathBuf;
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn write_state(dir: &PathBuf, body: &str) {
    std::fs::create_dir_all(dir.join(".metaform")).expect("create the project's directory");
    std::fs::write(dir.join(".metaform").join("state.json"), body).expect("write the state");
}

#[test]
fn the_axes_are_enumerated_and_the_legal_ones_are_counted() {
    let dir = std::env::temp_dir().join(format!("metaform-axes-{}-a", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // `a` covers {0,1,2}, `b` covers {0,3}; object 0 is a strong positive and object 2 a strong
    // negative. The conjunction `a & b` is {0}: it contains the positive and no negative, so it is
    // legal, and it differs from both predicates in force, so it counts as coverage.
    write_state(
        &dir,
        r#"{"n":4,"predicates":[{"name":"a","ids":[0,1,2]},{"name":"b","ids":[0,3]}],
            "votes":[{"id":0,"yes":3,"no":0},{"id":2,"yes":0,"no":3}]}"#,
    );
    let out = Command::new(ENGINE)
        .args(["axes", "list", "--project", dir.to_str().unwrap()])
        .output()
        .expect("run the command");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "the command failed: {text}");
    assert!(text.contains("\"legal_axes\""), "{text}");
    assert!(text.contains("\"coverage\":1"), "expected one new legal axis: {text}");
    assert!(text.contains("\"equivalence\""), "{text}");
}

#[test]
fn an_axis_that_contradicts_the_evidence_is_not_legal() {
    let dir = std::env::temp_dir().join(format!("metaform-axes-{}-b", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // `not a` is {3}; it contains no positive and no negative, so it is legal but trivial -- and the
    // command must not count a predicate that contains the negative object 2.
    write_state(
        &dir,
        r#"{"n":4,"predicates":[{"name":"a","ids":[0,1,2]}],
            "votes":[{"id":0,"yes":3,"no":0},{"id":2,"yes":0,"no":3}]}"#,
    );
    let out = Command::new(ENGINE)
        .args(["axes", "list", "--project", dir.to_str().unwrap()])
        .output()
        .expect("run the command");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(out.status.success(), "{text}");
    // `not a` contains object 2, which is a negative, so it must not be counted as legal.
    assert!(!text.contains("\"not a\",\"objects\":[3]"), "{text}");
}
