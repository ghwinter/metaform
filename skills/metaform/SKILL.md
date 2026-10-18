---
name: metaform
description: Decide that the current vocabulary cannot express this set of examples, then say which predicate to add and which object to ask. Use for concept discovery, predicate proposals, and diagnosing induction failures.
---

# metaform

A tool that answers three questions:

1. Can the current examples be expressed by the current vocabulary? (No ⇒ `INSUFFICIENT`; stop searching.)
2. Which predicates are missing? (`cover`: cover all positive/negative example pairs.)
3. Whom should we ask? (Merge conflicting objects by equivalence class to get the minimum number of queries.)

## When to use it

- Induction/fitting keeps failing and you suspect "not enough words" rather than "not enough search"
- You have a pile of candidate predicates and need to know which ones actually separate positives from negatives
- You need to turn "why I judged it this way" into reviewable evidence

## CLI

```powershell
metaform init --project .
metaform state init --project . --n 12
metaform state add  --project . --name overlap --ids 3,4,7,10
metaform state vote --project . --id 3 --yes 3 --no 0
metaform state vote --project . --id 4 --yes 0 --no 3 --record --by probe --reason "probe: collides with 3"
metaform state need --project .          # {"status":"INSUFFICIENT","action":"PROPOSE",...}
metaform ledger add --project . --case 3 --by overlap --reason "merge occurred"
metaform ledger cases --project .        # confirmed and conflict-free
metaform ledger conflicts --project .    # truth undetermined; re-ask needed
metaform plan --project . --pool pool.txt # coverage + purity + budget
metaform report --project .              # generate out/report.md (human-readable)
metaform score --project . --pool cands.txt --truth truth.txt  # scoring
metaform ops                              # schedule comparison of the five operator types (paper §4)
```

## MCP

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"state_need","arguments":{"project":"."}}}
```

Tools: `state_need`, `state_vote`, `plan`, `ledger_cases`, `ledger_conflicts`.
Start: `metaform mcp` (stdio, newline-delimited JSON).

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
