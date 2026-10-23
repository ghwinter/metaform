//! The state file carries a version so a future format cannot be half-read.

use std::path::PathBuf;

use metaform::state::{State, SCHEMA_VERSION};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("state-schema-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch");
    dir.join("state.json")
}

#[test]
fn a_file_without_a_version_loads_and_is_upgraded_in_memory() {
    let path = scratch("legacy");
    std::fs::write(&path, r#"{"n":3,"predicates":[],"votes":[]}"#).expect("write legacy state");
    let state = State::load(&path).expect("a pre-version file must still load");
    assert_eq!(state.n, 3);
}

#[test]
fn saving_stamps_the_current_version() {
    let path = scratch("stamp");
    let state = State { n: 2, ..State::default() };
    state.save(&path).expect("save");
    let text = std::fs::read_to_string(&path).expect("read back");
    assert!(
        text.contains(&format!("\"schema\": {SCHEMA_VERSION}")),
        "the file must state its format: {text}"
    );
    let reloaded = State::load(&path).expect("reload");
    assert_eq!(reloaded.n, 2);
}

#[test]
fn a_file_from_the_future_is_refused_with_the_versions_named() {
    let path = scratch("future");
    std::fs::write(
        &path,
        format!(r#"{{"schema":{},"n":1,"predicates":[],"votes":[]}}"#, SCHEMA_VERSION + 1),
    )
    .expect("write future state");
    let error = State::load(&path).expect_err("a future format must not be read");
    assert!(error.contains("newer than this binary understands"), "{error}");
    assert!(error.contains(&(SCHEMA_VERSION + 1).to_string()), "{error}");
}
