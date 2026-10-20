//! `metaform apply`: validate a proposal response and adopt its accepted candidates in one step.
//!
//! Before this command the caller had to run `propose`, read the accepted ids, and run `state add`
//! once per candidate. Those steps can drift: the state can gain a name between the proposal and
//! adoption, and a definition can be adopted with the wrong shape. `apply` keeps the validation and
//! the adoption in one process: it calls the same validator as `propose`, checks every accepted
//! candidate against the *current* state, adopts the survivors exactly as `state add` would, and
//! writes `.metaform/state.json` once at the end.
//!
//! A candidate is skipped, not adopted, when its name already exists in the state (`name_exists`);
//! candidates the validator itself rejected are reported with the validator's reason. Either way
//! the rest of the response is still applied. A whole-response rejection (wrong protocol, wrong
//! request id, malformed JSON) exits non-zero and changes nothing.
//!
//! Definitions are stored as definitions, not as the extension `propose` computed: the state file
//! keeps `{"definition":{"op":...,"operands":[...]}}` and recomputes the extension on load, exactly
//! like `state add --define`. Every adoption is first performed on a copy of the state, so a
//! definition that cannot resolve leaves the file byte-for-byte unchanged.
//!
//! The output is one JSON line with a fixed field order. The request and response files are only
//! read, and the before/after status uses the same recommended-`theta` view as `suggest` and
//! `state need`.

use std::collections::HashSet;

use metaform::propose::{self, Accepted};
use metaform::suggest;

use crate::state::PredSpec;

/// `metaform apply --project <dir> --request <request.json> --response <response.json>`
pub fn cmd(args: &[String]) -> i32 {
    let (Some(request_path), Some(response_path)) =
        (crate::kv(args, "--request"), crate::kv(args, "--response"))
    else {
        eprintln!(
            "usage: metaform apply --project <dir> --request <request.json> --response <response.json>"
        );
        return 2;
    };

    let root = crate::project_root(args).join(".metaform");
    let path = root.join("state.json");
    let Some(st) = crate::load_state(&path, "apply") else {
        return 2;
    };

    let request_text = match std::fs::read_to_string(&request_path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("metaform apply: cannot read request file `{request_path}`: {e}");
            return 2;
        }
    };
    // A missing or unreadable response is a protocol failure, exactly as in `propose`.
    let response_text = std::fs::read_to_string(&response_path).unwrap_or_default();
    let verdict = match propose::validate_response(&request_text, &response_text) {
        Ok(verdict) => verdict,
        Err(e) => {
            eprintln!("metaform apply: invalid request in `{request_path}`: {e}");
            return 2;
        }
    };

    let (before_status, before_colliding) = suggest::status_and_colliding(&st.explorer());

    // A whole-response rejection is not a partial adoption: print the same verdict `propose` would
    // print, and leave every byte of the state file alone.
    if let Some(reason) = &verdict.whole_reject {
        println!("{}", propose::verdict_json(&verdict));
        eprintln!("metaform apply: response rejected as a whole ({reason}); state unchanged");
        return 1;
    }

    // Everything the validator rejected is reported as skipped, with its reason. Accepted candidates
    // are then checked against the state that exists *now*, because the request may predate a name.
    let mut skipped: Vec<(String, String)> = verdict
        .rejected
        .iter()
        .map(|candidate| (candidate.name.clone(), candidate.reason.clone()))
        .collect();
    let mut current_names: HashSet<String> = st.predicates.iter().map(|p| p.name.clone()).collect();
    let mut adopted: Vec<Accepted> = Vec::new();
    for candidate in verdict.accepted {
        if current_names.contains(&candidate.name) {
            skipped.push((candidate.name, "name_exists".to_string()));
            continue;
        }
        current_names.insert(candidate.name.clone());
        adopted.push(candidate);
    }

    // Apply every adoption to a copy first. A definition can fail to resolve when the state no
    // longer has an operand the request had; that failure must not leave earlier candidates saved.
    let mut next = st.clone();
    for candidate in &adopted {
        next.predicates.push(PredSpec {
            name: candidate.name.clone(),
            // A defined candidate stores its definition; `resolve` fills the extension cache.
            ids: if candidate.definition.is_some() {
                Vec::new()
            } else {
                candidate.ids.clone()
            },
            definition: candidate.definition.clone(),
        });
    }
    if let Err(e) = next.resolve() {
        eprintln!("metaform apply: cannot adopt the accepted candidates: {e}; state unchanged");
        return 1;
    }

    // One write, after every candidate has been accepted into the in-memory state. Only adopted
    // candidates contribute predictions: a candidate skipped as `name_exists` was never admitted.
    if !adopted.is_empty() {
        if let Err(e) = next.save(&path) {
            eprintln!("metaform apply: cannot write `{}`: {e}", path.display());
            return 2;
        }
        let predictions_path = root.join("predictions.tsv");
        if let Err(e) =
            crate::predictions::record(&predictions_path, &verdict.request_id, &adopted)
        {
            eprintln!(
                "metaform apply: cannot write `{}`: {e}",
                predictions_path.display()
            );
            return 2;
        }
    }

    let (after_status, after_colliding) = suggest::status_and_colliding(&next.explorer());
    println!(
        "{}",
        output_json(
            adopted.len(),
            &skipped,
            before_status,
            before_colliding,
            after_status,
            after_colliding,
            &verdict.request_id,
        )
    );
    0
}

fn skipped_json(skipped: &[(String, String)]) -> String {
    let items: Vec<String> = skipped
        .iter()
        .map(|(name, reason)| {
            format!(
                "{{\"name\":{},\"reason\":{}}}",
                propose::json_string(name),
                propose::json_string(reason)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// One line, fixed field order: `applied`, `skipped`, `before`, `after`, `request_id`.
fn output_json(
    applied: usize,
    skipped: &[(String, String)],
    before_status: metaform::Status,
    before_colliding: usize,
    after_status: metaform::Status,
    after_colliding: usize,
    request_id: &str,
) -> String {
    format!(
        "{{\"applied\":{applied},\"skipped\":{},\"before\":{{\"status\":\"{}\",\"colliding\":{before_colliding}}},\"after\":{{\"status\":\"{}\",\"colliding\":{after_colliding}}},\"request_id\":{}}}",
        skipped_json(skipped),
        suggest::status_str(before_status),
        suggest::status_str(after_status),
        propose::json_string(request_id)
    )
}
