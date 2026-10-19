//! Minimal MCP (Model Context Protocol) server: stdio + line-delimited JSON-RPC.
//!
//! Why it exists: the CLI is the interface 'a human can type'; MCP is the interface 'an agent can mount'.
//! The same operations (read state, read ledger, record a vote) go through the same kernel; only the transport differs.
//!
//! Supported methods:
//!   initialize / notifications/initialized / ping / tools/list / tools/call
//!
//! Tools:
//!   state_need       read .metaform/state.json and return the state machine JSON (same as the CLI's need)
//!   state_vote       record a vote (writes state.json)
//!   ledger_cases     objects that are confirmed and conflict-free
//!   ledger_conflicts objects with conflicting directions

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::ledger;
use crate::plan;
use crate::state;

const PROTOCOL: &str = "2024-11-05";

fn meta_dir(args: &Value) -> PathBuf {
    let project = args
        .get("project")
        .and_then(|v| v.as_str())
        .unwrap_or(".");
    Path::new(project).join(".metaform")
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
    let id = args.get("id").and_then(|v| v.as_u64()).ok_or("missing id")? as usize;
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
    let pool: Option<Vec<(String, u64)>> = args
        .get("pool")
        .and_then(|v| v.as_str())
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| plan::parse_pool(&text));
    Ok(plan::to_json(&plan::build(&e, &rows, theta, pool.as_deref())))
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
            "description": "Coverage + purity + re-ask budget: greedily select predicates that fill gaps, and estimate p and rework cost R from the ledger.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "theta": {"type": "integer"},
                    "pool": {"type": "string", "description": "Path to the candidate predicate pool file"}
                }
            }
        },
        {
            "name": "ledger_conflicts",
            "description": "Return the ids of objects with a direction conflict and undetermined truth.",
            "inputSchema": {"type": "object", "properties": {"project": {"type": "string"}}}
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
            "protocolVersion": PROTOCOL,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "metaform", "version": env!("CARGO_PKG_VERSION")}
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match call_tool(name, &args) {
                Ok(text) => Ok(json!({"content": [{"type": "text", "text": text}]})),
                Err(e) => Ok(json!({
                    "content": [{"type": "text", "text": e}],
                    "isError": true
                })),
            }
        }
        other => Err(((-32601), format!("method not found: {other}"))),
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

    #[test]
    fn initialize_reports_protocol_and_tools_capability() {
        let req = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
        let resp = handle(&req).expect("initialize must have a reply");
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL);
        assert!(resp["result"]["capabilities"]["tools"].is_object());
    }

    #[test]
    fn tools_list_exposes_five_tools() {
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
            vec!["state_need", "state_vote", "ledger_cases", "plan", "ledger_conflicts"]
        );
    }

    #[test]
    fn unknown_tool_returns_error_content() {
        let req = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
                         "params": {"name": "nope", "arguments": {}}});
        let resp = handle(&req).unwrap();
        assert_eq!(resp["result"]["isError"], true);
    }

    #[test]
    fn notifications_get_no_reply() {
        let req = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert!(handle(&req).is_none());
    }
}



