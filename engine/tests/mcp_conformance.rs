//! Wire-level MCP conformance: one process, one newline-delimited transcript.
//!
//! The host probes `server/discover` first; a `-32601` error with no `result` key is what tells it
//! that this is a legacy server, after which it runs the ordinary `initialize` handshake. This test
//! feeds that sequence and the normal tool/resource traffic through the real binary over stdin, and
//! freezes the exact response sequence. Blank lines are part of the input on purpose: the stdio loop
//! must skip them without producing a reply.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

const ENGINE: &str = env!("CARGO_BIN_EXE_metaform");

fn workspace_tmp(tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine lives under the workspace root")
        .join("tmp");
    std::fs::create_dir_all(&root).expect("create workspace tmp");
    let dir = root.join(format!("mcp-conformance-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".metaform")).expect("create scratch project");
    dir
}

#[test]
fn stdio_transcript_matches_the_host_conformance_sequence() {
    let project = workspace_tmp("stdio");
    std::fs::write(
        project.join(".metaform").join("state.json"),
        serde_json::to_string_pretty(&json!({
            "n": 2,
            "predicates": [{"name": "a", "ids": [0]}],
            "votes": [
                {"id": 0, "yes": 3, "no": 0},
                {"id": 1, "yes": 0, "no": 3}
            ]
        }))
        .expect("serialize state"),
    )
    .expect("write state");

    let discover = json!({"jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": {}});
    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "conformance", "version": "1"}
        }
    });
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let list = json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"});
    let need = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "state_need",
            "arguments": {"project": project.to_str().expect("utf-8 scratch path")}
        }
    });
    let unknown_tool = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {"name": "nope", "arguments": {}}
    });
    let resources = json!({"jsonrpc": "2.0", "id": 6, "method": "resources/list"});

    let requests = [
        discover,
        initialize,
        initialized,
        list,
        need,
        unknown_tool,
        resources,
    ];
    let mut transcript = String::new();
    transcript.push_str("\n");
    for request in &requests {
        transcript.push_str(&serde_json::to_string(request).expect("serialize request"));
        transcript.push_str("\n\n");
    }
    transcript.push_str("not json\n\n");

    let mut child = Command::new(ENGINE)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn `metaform mcp`");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(transcript.as_bytes())
        .expect("write the transcript");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for `metaform mcp`");
    assert!(
        output.status.success(),
        "`metaform mcp` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        7,
        "one reply per id; notifications and blank lines get none: {stdout}"
    );
    let replies: Vec<Value> = lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("every stdout line is JSON"))
        .collect();

    // 1. Legacy probe: -32601 and deliberately no result key.
    assert_eq!(replies[0]["error"]["code"], -32601, "{}", replies[0]);
    assert!(
        replies[0].get("result").is_none(),
        "the host reads a result key as a modern discovery answer: {}",
        replies[0]
    );

    // 2. initialize echoes the offered protocol version.
    assert_eq!(replies[1]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[1]["result"]["serverInfo"]["name"], "metaform");
    assert!(replies[1]["result"]["capabilities"]["tools"].is_object());

    // 3. tools/list is the complete ten-tool set.
    let names: Vec<&str> = replies[2]["result"]["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name is a string"))
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

    // 4. state_need returns one text block carrying the same JSON the CLI prints.
    assert_eq!(replies[3]["result"]["content"][0]["type"], "text");
    let need_text = replies[3]["result"]["content"][0]["text"]
        .as_str()
        .expect("state_need text");
    let need_json: Value = serde_json::from_str(need_text).expect("state_need text is JSON");
    assert_eq!(need_json["status"], "IDENTIFIED");
    assert_eq!(need_json["action"], "STOP");

    // 5. An unknown tool is a tool error, never a JSON-RPC error and never a `result.error` key.
    assert_eq!(replies[4]["result"]["isError"], true);
    assert!(replies[4].get("error").is_none());
    assert!(replies[4]["result"].get("error").is_none());
    assert_eq!(
        replies[4]["result"]["content"][0]["text"],
        "unknown tool: nope"
    );

    // 6. resources/list is unsupported and takes the same method-not-found shape as discovery.
    assert_eq!(replies[5]["error"]["code"], -32601);
    assert!(replies[5].get("result").is_none());
    assert!(replies[5]["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("resources/list"));

    // 7. A malformed line is a parse error with a null id and no result.
    assert!(replies[6].get("id").is_some());
    assert!(replies[6]["id"].is_null());
    assert_eq!(replies[6]["error"]["code"], -32700);
    assert!(replies[6].get("result").is_none());

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn version_and_help_are_answered_without_starting_a_script() {
    // Before this was handled, `--version` fell through to the script runner and panicked trying
    // to open a file called `--version`, which is what an install check runs first.
    let version = Command::new(ENGINE).arg("--version").output().expect("run --version");
    assert!(version.status.success(), "--version must exit 0");
    let text = String::from_utf8_lossy(&version.stdout);
    assert!(text.starts_with("metaform "), "unexpected version line: {text}");
    assert!(text.trim().split(' ').nth(1).unwrap().chars().next().unwrap().is_ascii_digit());

    let help = Command::new(ENGINE).arg("--help").output().expect("run --help");
    assert!(help.status.success(), "--help must exit 0");
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("usage:"), "help must say how to use it: {text}");
    assert!(text.contains("mcp"), "help must mention the mcp command");
}
