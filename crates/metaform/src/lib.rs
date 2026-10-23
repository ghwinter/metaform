//! Query-driven representation change: the core state machine (the executable counterpart of the
//! paper's formalization).
//!
//! The measure itself lives in `metaform-kernel`; this crate is the protocol, the controller and the
//! interfaces, and it re-exports the kernel so a caller still sees one crate.

pub use metaform_kernel::*;

/// Multi-operator typing (paper §4): five classes of steps share one measure `Φ`, and each class is checked for what it can change.
pub mod operators;

/// `propose`: the operator protocol -- a request, a response, and the verdict that decides
/// whether a candidate may be adopted. A crate of its own, re-exported under the old name so
/// `metaform::propose::validate_response` keeps working.
pub use metaform_propose as propose;

/// `suggest`: enumerate Boolean definitions over the vocabulary and report why none can break a collision.
pub mod suggest;

/// `explore`: attribute exploration over the predicate context, ported from conexp-clj.
pub mod explore;

/// `coverage`: read-only closure over a goal-coverage table (items x required properties).
pub mod coverage;

/// Portfolio identification: several vocabularies, exactly one of which is sufficient (paper Theorem 2).
pub mod portfolio;

/// Vocabulary change as a morphism: fidelity, gain, and the collision a merge can hide.
pub mod morphism;

/// Coverage + purity + re-ask budget for one state.
pub mod plan;

/// The four files a project keeps in its `.metaform` directory, as one crate: state, laws, ledger,
/// predictions. Re-exported module by module, so `metaform::state::State` keeps its name.
pub use metaform_store::{laws, ledger, predictions, state};

/// The `explore --emit-questions` -> `answer` loop.
pub mod answer;

/// Adoption of a validated `metaform.propose/1` response.
pub mod apply;

/// Minimal MCP stdio server.
pub mod mcp;
pub mod metrics;
pub mod check;
/// `check` subcommands that assert something about repository files: `text`, `count`, `fresh`,
/// `absent`, `members`.
pub mod inspect;

/// Read the value of a `--flag value` pair, if present.
pub fn kv(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}

/// Parse a flag value while keeping "absent" distinct from "present but missing". A following
/// flag is not accepted as a value, so `--laws --out x` is an error rather than a file named
/// `--out`.
pub fn flag_value(args: &[String], key: &str) -> Result<Option<String>, String> {
    match args.iter().position(|a| a == key) {
        None => Ok(None),
        Some(i) => match args.get(i + 1) {
            Some(value) if !value.starts_with("--") => Ok(Some(value.clone())),
            _ => Err(format!("{key} expects a value")),
        },
    }
}

/// The project root selected by `--project` (default `.`).
pub fn project_root(args: &[String]) -> std::path::PathBuf {
    std::path::PathBuf::from(kv(args, "--project").unwrap_or_else(|| ".".into()))
}

/// Load the project state, reporting an unresolvable definition (missing operand, cycle, bad file)
/// on stderr and returning `None` so the caller can exit with status 2.
pub fn load_state(path: &std::path::Path, command: &str) -> Option<state::State> {
    match state::State::load(path) {
        Ok(st) => Some(st),
        Err(e) => {
            eprintln!("metaform {command}: {e}");
            None
        }
    }
}

