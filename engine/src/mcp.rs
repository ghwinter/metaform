//! Minimal MCP (Model Context Protocol) server: stdio + line-delimited JSON-RPC.
//!
//! Why it exists: the CLI is the interface 'a human can type'; MCP is the interface 'an agent can mount'.
//! The same operations (read state, read ledger, record a vote) go through the same kernel; only the transport differs.
//!
//! Supported methods:
//!   initialize / notifications/initialized / ping / tools/list / tools/call
//!
//! Protocol negotiation: `initialize` echoes the client's `params.protocolVersion` when it is one
//! of the revisions this server implements, and answers the newest supported revision otherwise.
//! The version is not pinned, so a host that negotiates a modern revision and then falls back to a
//! legacy one sees the same server.
//!
//! Tools:
//!   state_need           read .metaform/state.json and return the state machine JSON (same as the CLI's need)
//!   state_vote           record a vote (writes state.json)
//!   ledger_cases         objects that are confirmed and conflict-free
//!   ledger_conflicts     objects with conflicting directions
//!   plan                 coverage + purity + re-ask budget (the pool path stays inside the project)
//!   suggest_emit_request build the ready-to-fill `metaform.propose/1` request
//!   apply_response       validate and adopt a `metaform.propose/1` response
//!   check_predictions    score recorded held-out predictions
//!   explore_questions    run attribute exploration and return the open questions
//!   answer_questions     append expert answers as laws/refutations (replay-safe)
//!
//! Every result is text-only JSON: one `{"type":"text","text":...}` content block.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::{answer, apply, explore, laws, ledger, plan, predictions, state, suggest};

/// The protocol revisions this server can echo during `initialize`.
const SUPPORTED_PROTOCOLS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// The revision answered when the client asks for one this server does not implement.
const FALLBACK_PROTOCOL: &str = "2025-11-25";

/// Echo the client's requested revision when it is supported, otherwise answer the fallback.
///
/// MCP hosts negotiate by offering a revision and checking what the server returns; a server that
/// pins one version refuses every host whose supported set does not intersect the pin.
fn negotiated_protocol(params: &Value) -> String {
    match params.get("protocolVersion").and_then(|v| v.as_str()) {
        Some(version) if SUPPORTED_PROTOCOLS.contains(&version) => version.to_string(),
        _ => FALLBACK_PROTOCOL.to_string(),
    }
}

/// The project root supplied by the caller; defaults to the server's current directory.
fn project_path(args: &Value) -> PathBuf {
    PathBuf::from(args.get("project").and_then(|v| v.as_str()).unwrap_or("."))
}

fn meta_dir(args: &Value) -> PathBuf {
    project_path(args).join(".metaform")
}

/// Read an optional non-negative integer argument.
fn arg_usize(args: &Value, key: &str, default: usize) -> Result<usize, String> {
    match args.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .map(|n| n as usize)
            .ok_or_else(|| format!("`{key}` must be a non-negative integer")),
    }
}

fn tool_state_need(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let st = state::State::load(&dir.join("state.json"))?;
    let theta = args.get("theta").and_then(|v| v.as_u64()).map(|x| x as u8);
    Ok(state::need_json(&st.explorer(), theta))
}

fn tool_state_vote(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let path = dir.join("state.json");
    let mut st = state::State::load(&path)?;
    let id = args
        .get("id")
        .and_then(|v| v.as_u64())
        .ok_or("missing id")? as usize;
    let yes = args.get("yes").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    let no = args.get("no").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
    st.votes.push(state::VoteSpec { id, yes, no });
    st.save(&path).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true, "id": id, "yes": yes, "no": no}).to_string())
}

fn tool_ledger_cases(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let rows = ledger::read_all(&dir.join("ledger.tsv"));
    let cases = ledger::confirmed_cases(&rows, 0.5, 0.7);
    Ok(json!({"cases": cases}).to_string())
}

fn tool_ledger_conflicts(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let rows = ledger::read_all(&dir.join("ledger.tsv"));
    let conf = ledger::conflicts(&rows, 0.5, 0.7);
    let cases: Vec<usize> = conf.keys().copied().collect();
    Ok(json!({"conflicts": cases}).to_string())
}

/// Canonical project root, used when a tool accepts a path from the model.
fn canonical_project(args: &Value) -> Result<PathBuf, String> {
    let raw = args.get("project").and_then(|v| v.as_str()).unwrap_or(".");
    std::fs::canonicalize(raw).map_err(|e| format!("cannot resolve project root `{raw}`: {e}"))
}

/// A pool path is model-supplied. Resolve it first, then require it to stay under the project root;
/// a path that escapes the project is refused before any file is read. A relative path is resolved
/// against the project root, not the server's working directory.
fn confined_pool(args: &Value) -> Result<Option<PathBuf>, String> {
    let Some(raw) = args.get("pool").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let root = canonical_project(args)?;
    let candidate = Path::new(raw);
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|e| format!("cannot resolve pool path `{raw}`: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err("pool path must stay inside the project root".into());
    }
    Ok(Some(canonical))
}

fn tool_plan(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let st = state::State::load(&dir.join("state.json"))?;
    let rows = ledger::read_all(&dir.join("ledger.tsv"));
    let e = st.explorer();
    let theta = args
        .get("theta")
        .and_then(|v| v.as_u64())
        .map(|x| x as u8)
        .unwrap_or_else(|| e.recommended_theta());
    let pool: Option<Vec<(String, u64)>> = match confined_pool(args)? {
        Some(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read pool `{}`: {e}", path.display()))?;
            Some(plan::parse_pool(&text))
        }
        None => None,
    };
    Ok(plan::to_json(&plan::build(
        &e,
        &rows,
        theta,
        pool.as_deref(),
    )))
}

fn tool_suggest_emit_request(args: &Value) -> Result<String, String> {
    let dir = meta_dir(args);
    let st = state::State::load(&dir.join("state.json"))?;
    let (status, request) = suggest::request_for_state(&st.explorer());
    let request: Value = match request {
        Some(text) => serde_json::from_str(&text)
            .map_err(|e| format!("the request builder returned invalid JSON: {e}"))?,
        None => Value::Null,
    };
    Ok(json!({"status": suggest::status_str(status), "request": request}).to_string())
}

fn tool_apply_response(args: &Value) -> Result<String, String> {
    let request = args.get("request").ok_or("missing request")?;
    let response = args.get("response").ok_or("missing response")?;
    let request =
        serde_json::to_string(request).map_err(|e| format!("request is not valid JSON: {e}"))?;
    let response =
        serde_json::to_string(response).map_err(|e| format!("response is not valid JSON: {e}"))?;
    apply::apply_text(&project_path(args), &request, &response).map_err(|failure| failure.message)
}

fn tool_check_predictions(args: &Value) -> Result<String, String> {
    predictions::check(&project_path(args))
}

/// Run attribute exploration for one project and return the open questions JSON.
///
/// `include_expert` is false when the caller is about to append the answers it is replaying: an
/// expert entry already on disk is a previous answer, so leaving it out of the rebuilt question
/// list keeps the question ids stable across a replay. `explore_questions` passes true, so it sees
/// exactly the theory the CLI's `explore` sees.
fn explore_questions_for(
    project: &Path,
    max_questions: usize,
    include_expert: bool,
) -> Result<String, String> {
    let dir = project.join(".metaform");
    let st = state::State::load(&dir.join("state.json"))?;
    if st.n > 64 {
        return Err(format!(
            "at most 64 objects are supported (state has {})",
            st.n
        ));
    }
    let e = st.explorer();
    let theta = e.recommended_theta();
    let (sp, sn) = e.strong(theta);
    let context = explore::Context::from_explorer(&e, sp | sn)?;
    let loaded = laws::load(&dir.join("laws.tsv"))?;
    let loaded = if include_expert {
        loaded
    } else {
        loaded
            .into_iter()
            .filter(|entry| entry.source != "expert")
            .collect()
    };
    let (theory, recorded) = laws::masks(&loaded, &context.names)?;
    let refutations: Vec<explore::Refutation> = recorded
        .into_iter()
        .map(|entry| explore::Refutation {
            premise: entry.premise,
            conclusion: entry.conclusion,
            counterexample: entry.counterexample,
            source: entry.source,
        })
        .collect();
    let report = context.run_with_theory_and_refutations(
        max_questions,
        e.status_with(theta),
        &theory,
        &refutations,
    );
    Ok(explore::questions_json(&report))
}

fn tool_explore_questions(args: &Value) -> Result<String, String> {
    let max = arg_usize(args, "max_questions", explore::DEFAULT_BUDGET)?;
    explore_questions_for(&project_path(args), max, true)
}

fn tool_answer_questions(args: &Value) -> Result<String, String> {
    let answers = args
        .get("answers")
        .and_then(|v| v.as_str())
        .ok_or("missing answers")?;
    let max = arg_usize(args, "max_questions", explore::DEFAULT_BUDGET)?;
    let project = project_path(args);
    let questions = explore_questions_for(&project, max, false)?;
    answer::apply_text(&project, None, &questions, answers)
}

fn tools() -> Value {
    json!([
        {
            "name": "state_need",
            "description": "Read the project's .metaform/state.json and return the current status: INSUFFICIENT/UNDETERMINED/IDENTIFIED, colliding pairs, and suggested objects to label.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string", "description": "Project root directory; defaults to ."},
                    "theta": {"type": "integer", "description": "Vote-margin threshold; defaults to automatic"}
                }
            }
        },
        {
            "name": "state_vote",
            "description": "Record one vote (yes/no) for an object and write it to .metaform/state.json.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "id": {"type": "integer"},
                    "yes": {"type": "integer"},
                    "no": {"type": "integer"}
                },
                "required": ["id"]
            }
        },
        {
            "name": "ledger_cases",
            "description": "Return the ids of objects that are confirmed and have no direction conflict (from .metaform/ledger.tsv).",
            "inputSchema": {"type": "object", "properties": {"project": {"type": "string"}}}
        },
        {
            "name": "plan",
            "description": "Coverage + purity + re-ask budget: greedily select predicates that fill gaps, and estimate p and rework cost R from the ledger. The optional pool path must resolve inside the project root.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "theta": {"type": "integer"},
                    "pool": {"type": "string", "description": "Path to the candidate predicate pool file inside the project root"}
                }
            }
        },
        {
            "name": "ledger_conflicts",
            "description": "Return the ids of objects with a direction conflict and undetermined truth.",
            "inputSchema": {"type": "object", "properties": {"project": {"type": "string"}}}
        },
        {
            "name": "suggest_emit_request",
            "description": "Read-only: return {\"status\":...,\"request\":object|null}, the ready-to-fill metaform.propose/1 request for an INSUFFICIENT state.",
            "inputSchema": {
                "type": "object",
                "properties": {"project": {"type": "string"}}
            }
        },
        {
            "name": "apply_response",
            "description": "Validate a metaform.propose/1 request/response pair and adopt the accepted candidates, returning applied/skipped/before/after JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "request": {"type": "object"},
                    "response": {"type": "object"}
                },
                "required": ["request", "response"]
            }
        },
        {
            "name": "check_predictions",
            "description": "Read-only: score the predictions recorded in .metaform/predictions.tsv, returning checked/correct/wrong/open.",
            "inputSchema": {
                "type": "object",
                "properties": {"project": {"type": "string"}}
            }
        },
        {
            "name": "explore_questions",
            "description": "Read-only: run attribute exploration over the current state and laws, returning {status,questions:[{id,premise,conclusion,text}]}.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "max_questions": {"type": "integer", "description": "Premises examined before stopping; defaults to 1000"}
                }
            }
        },
        {
            "name": "answer_questions",
            "description": "Append expert answers to .metaform/laws.tsv after rebuilding the open questions; replaying the same answers does not duplicate an entry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "max_questions": {"type": "integer"},
                    "answers": {"type": "string", "description": "JSON text in the metaform answer format, e.g. {\"answers\":[{\"id\":0,\"answer\":\"yes\"}]}"}
                },
                "required": ["answers"]
            }
        }
    ])
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "state_need" => tool_state_need(args),
        "state_vote" => tool_state_vote(args),
        "ledger_cases" => tool_ledger_cases(args),
        "ledger_conflicts" => tool_ledger_conflicts(args),
        "plan" => tool_plan(args),
        "suggest_emit_request" => tool_suggest_emit_request(args),
        "apply_response" => tool_apply_response(args),
        "check_predictions" => tool_check_predictions(args),
        "explore_questions" => tool_explore_questions(args),
        "answer_questions" => tool_answer_questions(args),
        other => Err(format!("unknown tool: {other}")),
    }
}

/// Handle one JSON-RPC request; notifications (no id) return None.
pub fn handle(req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));

    if id.is_none() {
        return None; // notification: no reply
    }
    let id = id.unwrap();

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": negotiated_protocol(&params),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "metaform", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        // The host probes `server/discover` first and treats a -32601 error *without* a result key
        // as a legacy server, then starts the normal `initialize` handshake. Never return a result
        // for this method: a result would claim the modern discovery protocol.
        "server/discover" => Err((-32601, "method not found: server/discover".to_string())),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(name, &args) {
                Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}]})),
                Err(e) => Ok(json!({
                    "content": [{"type": "text", "text": e}],
                    "isError": true
                })),
            }
        }
        other => Err((-32601, format!("method not found: {other}"))),
    };

    Some(match result {
        Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
        Err((code, message)) => json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": code, "message": message}
        }),
    })
}

/// stdio loop over line-delimited JSON. Exits on EOF.
pub fn serve() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            let err = json!({"jsonrpc": "2.0", "id": null,
                             "error": {"code": -32700, "message": "parse error"}});
            let _ = writeln!(out, "{err}");
            let _ = out.flush();
            continue;
        };
        if let Some(resp) = handle(&req) {
            let _ = writeln!(out, "{resp}");
            let _ = out.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(tag: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("engine lives under the workspace root")
            .join("tmp");
        fs::create_dir_all(&root).expect("create workspace tmp");
        let dir = root.join(format!("mcp-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".metaform")).expect("create scratch project");
        dir
    }

    fn write_state(project: &Path, value: Value) {
        fs::write(
            project.join(".metaform").join("state.json"),
            serde_json::to_string_pretty(&value).expect("serialize scratch state"),
        )
        .expect("write scratch state");
    }

    fn state_json(n: usize, predicates: Value, votes: Value) -> Value {
        json!({"n": n, "predicates": predicates, "votes": votes})
    }

    /// n objects, the listed predicates, then object 0 strong-positive and object 1 strong-negative.
    fn collision_fixture(tag: &str) -> PathBuf {
        let project = scratch(tag);
        write_state(
            &project,
            state_json(
                2,
                json!([
                    {"name": "a", "ids": [0, 1]},
                    {"name": "b", "ids": [0, 1]}
                ]),
                json!([
                    {"id": 0, "yes": 3, "no": 0},
                    {"id": 1, "yes": 0, "no": 3}
                ]),
            ),
        );
        project
    }

    fn call(name: &str, args: Value) -> Value {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": args}
        });
        handle(&req).expect("tools/call has a reply")
    }

    fn result_text(response: &Value) -> &str {
        response["result"]["content"][0]["text"]
            .as_str()
            .expect("tool results are text-only JSON")
    }

    fn result_json(response: &Value) -> Value {
        serde_json::from_str(result_text(response)).expect("tool result text is JSON")
    }

    fn initialize(params: Option<Value>) -> Value {
        let mut req = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
        if let Some(params) = params {
            req["params"] = params;
        }
        handle(&req).expect("initialize must have a reply")["result"].clone()
    }

    #[test]
    fn initialize_echoes_each_supported_protocol_version() {
        for version in SUPPORTED_PROTOCOLS {
            let result = initialize(Some(json!({"protocolVersion": version})));
            assert_eq!(result["protocolVersion"], version, "must echo {version}");
            assert!(result["capabilities"]["tools"].is_object());
        }
    }

    #[test]
    fn initialize_falls_back_for_an_unknown_protocol_version() {
        let result = initialize(Some(json!({"protocolVersion": "1999-01-01"})));
        assert_eq!(result["protocolVersion"], FALLBACK_PROTOCOL);
        assert_eq!(result["serverInfo"]["name"], "metaform");
    }

    #[test]
    fn initialize_answers_when_params_are_missing() {
        for params in [None, Some(json!({})), Some(json!({"protocolVersion": 7}))] {
            let result = initialize(params);
            assert_eq!(result["protocolVersion"], FALLBACK_PROTOCOL);
            assert!(result["capabilities"]["tools"].is_object());
        }
    }

    #[test]
    fn server_discover_is_method_not_found_without_a_result_key() {
        let req = json!({"jsonrpc": "2.0", "id": 9, "method": "server/discover", "params": {}});
        let response = handle(&req).expect("server/discover must have a reply");
        assert_eq!(response["error"]["code"], -32601);
        assert!(
            response.get("result").is_none(),
            "the host detects a legacy server by an error with no result: {response}"
        );
        assert!(response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("server/discover"));
    }

    #[test]
    fn tools_list_exposes_exactly_ten_tools() {
        let req = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
        let resp = handle(&req).unwrap();
        let names: Vec<String> = resp["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "state_need",
                "state_vote",
                "ledger_cases",
                "plan",
                "ledger_conflicts",
                "suggest_emit_request",
                "apply_response",
                "check_predictions",
                "explore_questions",
                "answer_questions",
            ]
        );
    }

    #[test]
    fn tool_names_and_schemas_stay_model_safe() {
        for tool in tools().as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            assert!(name.len() <= 64, "tool name too long: {name}");
            assert!(
                name.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "tool name must match [a-z0-9_]+: {name}"
            );
            assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
            assert!(
                tool["inputSchema"]["properties"].is_object(),
                "{name} needs typed properties"
            );
            assert!(!tool["description"].as_str().unwrap().is_empty(), "{name}");
        }
    }

    #[test]
    fn suggest_emit_request_round_trips() {
        let project = collision_fixture("suggest");
        let text = result_json(&call(
            "suggest_emit_request",
            json!({"project": project.to_str().unwrap()}),
        ));
        assert_eq!(text["status"], "INSUFFICIENT");
        assert_eq!(text["request"]["protocol"], "metaform.propose/1");
        assert!(text["request"]["request_id"].is_string());
    }

    #[test]
    fn apply_response_round_trips() {
        let project = collision_fixture("apply");
        let project_str = project.to_str().unwrap();
        let suggested = result_json(&call(
            "suggest_emit_request",
            json!({"project": project_str}),
        ));
        let request = suggested["request"].clone();
        let request_id = request["request_id"].as_str().unwrap().to_string();
        let response = json!({
            "protocol": "metaform.propose/1",
            "request_id": request_id,
            "candidates": [{"name": "only0", "ids": [0]}]
        });
        let applied = result_json(&call(
            "apply_response",
            json!({"project": project_str, "request": request, "response": response}),
        ));
        assert_eq!(applied["applied"], 1);
        assert_eq!(applied["skipped"], json!([]));
        assert_eq!(applied["before"]["status"], "INSUFFICIENT");
        assert_eq!(applied["after"]["status"], "IDENTIFIED");
        assert_eq!(applied["request_id"], request_id);
    }

    #[test]
    fn check_predictions_round_trips() {
        let project = scratch("predictions");
        write_state(
            &project,
            state_json(
                2,
                json!([{"name": "a", "ids": [0]}]),
                json!([
                    {"id": 0, "yes": 3, "no": 0},
                    {"id": 1, "yes": 0, "no": 3}
                ]),
            ),
        );
        fs::write(
            project.join(".metaform").join("predictions.tsv"),
            "request_id\tname\tid\tpredicted\textension\nr1\tp\t0\tpos\t0\n",
        )
        .unwrap();
        let checked = result_json(&call(
            "check_predictions",
            json!({"project": project.to_str().unwrap()}),
        ));
        assert_eq!(checked["checked"], 1);
        assert_eq!(checked["correct"], 1);
        assert_eq!(checked["wrong"], json!([]));
        assert_eq!(checked["open"], 0);
    }

    #[test]
    fn explore_questions_round_trips() {
        let project = scratch("explore");
        write_state(
            &project,
            state_json(
                4,
                json!([
                    {"name": "a", "ids": [2]},
                    {"name": "b", "ids": [3]}
                ]),
                json!([{"id": 0, "yes": 3, "no": 0}]),
            ),
        );
        let questions = result_json(&call(
            "explore_questions",
            json!({"project": project.to_str().unwrap(), "max_questions": 100}),
        ));
        assert!(questions["status"].is_string());
        let list = questions["questions"].as_array().unwrap();
        assert!(!list.is_empty());
        assert_eq!(list[0]["id"], 0);
        assert!(list[0]["premise"].is_array());
        assert!(list[0]["conclusion"].is_array());
        assert!(list[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Does every object"));
    }

    #[test]
    fn answer_questions_is_idempotent_on_replay() {
        let project = scratch("answer");
        write_state(
            &project,
            state_json(
                4,
                json!([
                    {"name": "a", "ids": [2]},
                    {"name": "b", "ids": [3]}
                ]),
                json!([{"id": 0, "yes": 3, "no": 0}]),
            ),
        );
        let args = json!({
            "project": project.to_str().unwrap(),
            "max_questions": 100,
            "answers": "{\"answers\":[{\"id\":0,\"answer\":\"yes\"}]}"
        });
        let first = result_json(&call("answer_questions", args.clone()));
        assert_eq!(first["answers"], 1);
        assert_eq!(first["laws"], 1);
        let laws_path = project.join(".metaform").join("laws.tsv");
        let after_first = fs::read(&laws_path).expect("laws file was written");

        let replay = result_json(&call("answer_questions", args));
        assert_eq!(replay["answers"], 1);
        assert_eq!(replay["laws"], 0);
        assert_eq!(replay["duplicates"], 1);
        assert_eq!(
            fs::read(&laws_path).expect("laws file still exists"),
            after_first,
            "replaying the same answers must leave the theory byte-for-byte unchanged"
        );
    }

    #[test]
    fn plan_refuses_a_pool_path_outside_the_project() {
        let project = collision_fixture("plan-outside");
        let outside = project
            .parent()
            .unwrap()
            .join(format!("outside-pool-{}.txt", std::process::id()));
        fs::write(&outside, "secret_pred 0\n").expect("write outside pool");
        let response = call(
            "plan",
            json!({
                "project": project.to_str().unwrap(),
                "pool": outside.to_str().unwrap()
            }),
        );
        assert_eq!(response["result"]["isError"], true, "{response}");
        assert!(
            response.get("error").is_none(),
            "tool failure is not a JSON-RPC error"
        );
        assert!(
            !result_text(&response).contains("secret_pred"),
            "the refused file's contents must not be returned: {response}"
        );
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn unknown_tool_returns_error_content() {
        let req = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                         "params": {"name": "nope", "arguments": {}}});
        let resp = handle(&req).unwrap();
        assert_eq!(resp["result"]["isError"], true);
        assert!(resp.get("error").is_none());
    }

    #[test]
    fn notifications_get_no_reply() {
        let req = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(handle(&req).is_none());
    }
}
