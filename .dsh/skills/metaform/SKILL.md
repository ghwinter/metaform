---
name: metaform
description: Decide that the current vocabulary cannot express this set of examples, then say which predicate to add and which object to ask. Use for concept discovery, predicate proposals, and diagnosing induction failures.
---

# metaform

A tool that answers three questions:

1. Can the current examples be expressed by the current vocabulary? (No ⇒ `INSUFFICIENT`; stop searching.)
2. Which predicates are missing? (Cover all positive/negative example pairs.)
3. Whom should we ask? (Merge conflicting objects by equivalence class to get the minimum number of queries.)

## When to use it

- Induction/fitting keeps failing and you suspect "not enough words" rather than "not enough search"
- You have a pile of candidate predicates and need to know which ones actually separate positives from negatives
- You need to turn "why I judged it this way" into reviewable evidence

## MCP tools

The host registers this server under the name `metaform`, so the model-visible tools are:

| Model-visible tool | What it does |
|---|---|
| `mcp__metaform__state_need` | Read `.metaform/state.json` and return the status, colliding pairs, and suggested labels |
| `mcp__metaform__state_vote` | Record one vote and write it to `.metaform/state.json` |
| `mcp__metaform__ledger_cases` | Return confirmed, conflict-free objects from `.metaform/ledger.tsv` |
| `mcp__metaform__ledger_conflicts` | Return objects with a direction conflict |
| `mcp__metaform__plan` | Coverage + purity + re-ask budget; a pool path must stay inside the project root |
| `mcp__metaform__suggest_emit_request` | Build the ready-to-fill `metaform.propose/1` request |
| `mcp__metaform__apply_response` | Validate a proposal response and adopt its accepted candidates |
| `mcp__metaform__check_predictions` | Score the held-out predictions recorded in `.metaform/predictions.tsv` |
| `mcp__metaform__explore_questions` | Run attribute exploration and return the open questions |
| `mcp__metaform__answer_questions` | Append expert answers as laws or refutations; replaying them is safe |

Every tool returns text-only JSON. The server is started by the host as `metaform mcp` over stdio
(newline-delimited JSON-RPC); the same operations are also available from the command line.

## Handshake

```json
{"jsonrpc":"2.0","id":0,"method":"server/discover","params":{}}
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"example","version":"1"}}}
{"jsonrpc":"2.0","id":2,"method":"notifications/initialized"}
{"jsonrpc":"2.0","id":3,"method":"tools/list"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"state_need","arguments":{"project":"."}}}
```

The first request is the host's legacy probe: a `-32601` error with no `result` member means the
server predates modern discovery, so the host falls back to `initialize`. The server echoes any
well-formed `protocolVersion` it is offered, including revisions newer than the one it was built
against, and answers `2025-11-25` only when the client sends no usable string.

## Reading the output

| Field | Meaning |
|---|---|
| `INSUFFICIENT` / `PROPOSE` | the vocabulary is insufficient; propose new predicates |
| `UNDETERMINED` / `ASK` | some equivalence classes are still unlabeled; ask questions |
| `IDENTIFIED` / `VERIFY` | uniquely determined, but some members were never asked directly: probe before stopping |
| `IDENTIFIED` / `STOP` | exactly one candidate concept, with no inherited labels |
| `colliding_pairs` | positive/negative example pairs that collide |
| `unverified` | members of multi-member equivalence classes that were never asked directly (probe them on `VERIFY`) |
| `pending` | objects below the θ vote threshold; collect more votes first |

## Boundaries

- A predicate must have decidable membership (0/1 computable for a single object).
- It does not invent predicates; it only decides that invention is required and gives the coverage target.
- Noise is tolerated up to about 0.2 error per answer; beyond that, raise θ or re-ask.
