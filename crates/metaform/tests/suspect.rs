//! Which labels a single round decided, and therefore which ones a second visit could overturn.

use std::path::PathBuf;
use std::process::Command;

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn project(tag: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("metaform-suspect-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".metaform")).expect("create the project's directory");
    std::fs::write(dir.join(".metaform").join("state.json"), body).expect("write the state");
    dir
}

fn run(dir: &PathBuf) -> String {
    let out = Command::new(ENGINE)
        .args(["suspect", "list", "--project", dir.to_str().unwrap()])
        .output()
        .expect("run the command");
    assert!(out.status.success(), "the command failed");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(dir);
    text
}

#[test]
fn a_label_one_round_decided_is_a_suspect_and_a_split_one_is_not() {
    // 3:0 is a clean decision from one visit -- the shape a wrong label had in the noisy family.
    // 3:3 is already split, so the vote gap refuses to call it and it is not a suspect but a pending
    // object. 6:0 has had two visits; a second visit agreed, and that is what this command asks for.
    let text = run(&project(
        "a",
        r#"{"n":4,"predicates":[{"name":"a","ids":[0,1]}],
            "votes":[{"id":0,"yes":3,"no":0},{"id":1,"yes":3,"no":3},{"id":2,"yes":6,"no":0}]}"#,
    ));
    assert!(text.contains("\"suspects\":[{\"id\":0"), "object 0 is the only suspect: {text}");
    assert!(!text.contains("\"id\":1"), "a split label is not a suspect: {text}");
    assert!(!text.contains("\"id\":2"), "two visits took it out of suspicion: {text}");
}

#[test]
fn a_margin_below_the_threshold_is_not_a_suspect_either() {
    // 2:0 is decided by a margin the threshold will not call, so it is undetermined rather than
    // settled -- asking again is already the regime, and it does not need to be flagged.
    let text = run(&project(
        "b",
        r#"{"n":3,"predicates":[{"name":"a","ids":[0]}],
            "votes":[{"id":0,"yes":2,"no":0}]}"#,
    ));
    assert!(text.contains("\"suspects\":[]"), "a thin margin is pending, not suspect: {text}");
}
