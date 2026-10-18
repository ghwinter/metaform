# 10 Tool Architecture: From `qde` to `metaform`

The goal is not "wrapping a layer around the scripts", but: **a single binary that any project can call,
leaving the structure of the induction process in the project.**

## 1. Four repositories of the same kind: what to learn from each, what not to

Read the four repositories in [.repo/](../.repo/README.md).

| Repository | What it got right | What not to learn from it |
|---|---|---|
| `concepts` | clean data model: `Context` / `Concept` / `Lattice` support `fromdict/fromjson/tostring`, serialization is a first-class citizen | it assumes the attribute set is given; there is no "insufficient" decision |
| `conexp-clj` | **interaction protocol**: `explore-attributes` + counterexample handlers (complete / incomplete), separating "what to ask" from "how to answer" | the Clojure ecosystem, GUI-oriented; not suitable as an agent's kernel |
| `metagol` | minimal: a single 11 KB `metagol.pl` says clearly what metarule ILP is | the Prolog runtime; its compositional closure happens to prove that "composition produces no new atoms" |
| `popper` | **input contract**: three files — examples / background knowledge / bias; failure is feedback (learning from failures) | SWI-Prolog dependency, heavy; its language bias is still given |

**Three things worth absorbing**:

1. Like `concepts`, **state is serializable** (JSON/TSV into `.metaform/`, human-readable, diffable).
2. Like `conexp-clj`, **separate "what to ask" from "what to answer"** — our `need` already has this shape;
   keep strengthening it.
3. Like `popper`, **input is files and output is files**; the binary does not assume whether the caller is
   a human or an agent.

**What we do not absorb**: their search algorithms. Our criterion is not "search better"; it is exactly
"when search stalls, the vocabulary must be extended".

## 2. Architecture

```
metaform (Rust, single binary, no runtime dependencies)
├── lib:  Explorer / Status / Ledger (pure logic, testable)
└── bin:  CLI — reads scripts, reads/writes the project directory, outputs JSON
```

- **Rust is the only kernel.** Python keeps only two kinds: experiments (the paper's numbers) and one-off
  migration scripts.
- **The CLI is not the substance.** The substance is the library; the CLI is only the first front end. An
  MCP server / skill can be added later; they call the same library.
- **No external dependencies** (currently zero crate dependencies). Add `serde`/`rusqlite` when structured
  storage is needed; do not carry them in advance.

## 3. The artifact directory `.metaform/`

In any project, `metaform` writes to a single subdirectory:

```
<project>/.metaform/
├── config.toml       # project-level settings (θ, oracle type, version)
├── ledger.tsv        # evidence ledger: append-only, folds the latest state by id
├── state.json        # current scenario (objects/predicates/votes), replayable
├── logs/             # input and output of each call, for review
└── out/              # generated md / json reports
```

Conventions:

1. **Write only `.metaform/`**; do not pollute other places in the project.
2. `ledger.tsv` is **append-only**: retraction = append a `retracted` row with the same id; when folding,
   the later one wins. This satisfies "changeable" while not fearing corrupted concurrent writes.
3. All output goes to both `out/` and stdout (the agent reads stdout, humans read files).

## 4. Migration order

| Step | Content | Status |
|---|---|---|
| 0 | `qde` script interpreter | already there |
| 1 | package name `querydriven` → `metaform`; binary `metaform` | this round |
| 2 | `init` creates the directory; the `ledger` subcommand (add/list/retract/cases/conflicts) writes TSV | done |
| 3 | direction/conflicts/confirmed semantics move into Rust | done |
| 4 | `state.json` snapshots (`metaform state init/add/vote/need/show`) | done |
| 5 | `metaform mcp` (stdio JSON-RPC) + `skills/metaform/SKILL.md` | done |
| 6 | `plan` (coverage/purity/budget) and `report` move into Rust | done |
| 7 | v0.2.0 release bundle (exe + SKILL + VERSION) | done |
| 8 | `score` moves into Rust; v0.3.0 release bundle | done |

## 5. Bootstrapping

This repository is also just an ordinary project: `metaform init` generates `.metaform/` here, and after
that every judgment (what is missing, whom to ask, whether the evidence suffices) is read from that
directory. Changing versions = replacing the binary; the data does not move.

There is only one criterion for judging whether it is really useful: **will I open it on my own the next
time I think**. If not, it is still merely an appendage of the paper and needs more iteration.
