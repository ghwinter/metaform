# Command reference

The binary is `target\debug\metaform.exe` (a release build lands beside it). Paths below are
relative to the repository root. `metaform --help` lists the commands; this page says what they
mean.

There are two ways in. A **scenario script** is a few lines of text that describe one state and ask
what it licenses -- no project directory, no files written. A **project** is a directory with a
`.metaform/` inside it, where the state, the evidence ledger, the recorded laws and the held-out
predictions persist between commands.

## Scenario scripts

```powershell
cargo run --quiet -p metaform -- crates\metaform\scenarios\setcover.demo
```

Script commands: `objects N` / `add <name> <id...>` / `pos <id...>` / `neg <id...>` /
`vote <id> <yes> <no>` / `ask` / `status` / `explain` / `score <name> <id...>` / `capacity` /
`need [theta]` / `batch k` / `gen [pool.txt]` (generate a metarule-style candidate pool from the
current vocabulary) / `cover <pool.txt>` (greedily cover all colliding pairs).

`need` outputs a machine-readable status (including `action`: `PROPOSE` / `ASK` / `STOP`), the
vote-gap threshold `theta` (defaults to `theta_rec`), `epsilon_hat` estimated from the vote counts,
the recommended `theta_rec`, and `pending` objects that need more votes.
Under a noisy oracle it suppresses false triggers (see `experiments/RESULTS.md` §3.11: at ε=0.1,
θ=1 succeeds only 25% of the time, while θ=3 succeeds 98%).

## A project directory

`metaform` writes to exactly one subdirectory, `.metaform/`:

```powershell
metaform init   --project .                      # create .metaform/{config.toml,ledger.tsv,logs,out}
metaform ledger add     --project . --case 3 --by demo --reason "first piece of evidence"
metaform ledger list    --project . --all        # TSV, folded by id to the latest state
metaform ledger retract --project . --id L2 --reason "oracle answered wrong"
metaform ledger cases   --project .              # confirmed objects with no conflicts
metaform ledger conflicts --project .            # direction conflicts: {"conflicts":[...]}
metaform concept add    --project . --kind vsa --name "MBAT binding" --claim "..." --evidence "..."  # register what the repository absorbed
metaform concept list   --project .                # every concept in force, as JSON
metaform concept show   --project . --id C1        # one concept, as JSON
metaform concept retract --project . --id C1 --reason "..."   # withdraw it; the earlier row stays readable
metaform axes list    --project . [--max-arity 2]  # what the current vocabulary leaves open: the equivalence classes, the candidate readings that agree with the evidence, and how many of them are new rather than reworded
metaform state init  --project . --n 4            # persist the scenario to .metaform/state.json
metaform state add   --project . --name p --define all-of:a,b  # store p = a AND b as a definition
metaform state vote  --project . --id 0 --yes 3 --no 0
metaform state need  --project .                  # one-shot: status and recommendations directly
metaform apply  --project . --request request.json --response response.json # validate + adopt accepted candidates with one state write
metaform check-predictions --project .            # read-only: score recorded held-out predictions against the state's strong labels
metaform translate  --input <spec.json|->          # read-only: is this vocabulary change a loss, a rewording, or a real gain
metaform suggest    --project . [--max-arity 2] [--max 20] [--emit-request [--out <path>]] # read-only: Boolean candidates tried, and why none separates
metaform explore    --project . [--max-questions 1000] [--laws <path>] [--emit-questions <path>] [--out <path>] # read-only: implications confirmed by evidence, refuted with a counterexample, or open
metaform answer     --project . --questions <q.json> --answers <a.json> [--laws <path>] # record expert answers as laws (and adopt a define), so the next explore starts from them
metaform coverage   [--project .] [--format text] [<table.tsv>] # read-only: missing cells, implication conflicts, duplicate rows, blocking gaps
metaform plan   --project . --pool .metaform/out/pool.txt # coverage + purity + re-ask budget
metaform report --project .                         # generate .metaform/out/report.md
metaform score  --project . --pool cands.txt --truth truth.txt   # candidate scoring: coverage/P-R/ledger
metaform mcp                                       # MCP server (stdio) for agents to mount
metaform check all                                 # repository checks, in the binary
metaform metrics verdict <rows.tsv>                # precision on answered, unknown rate, selective accuracy, AURC
metaform metrics bets <predictions.tsv>            # share of recorded bets placed where two rules disagree
metaform metrics readings --vocab <tsv> --readings <tsv>  # complement coverage and axis equivalence rate
metaform ops                                       # five-operator schedule comparison: fixed order vs skipping redundant steps (paper §4)
```

The ledger is **append-only**: retracting means appending a `retracted` line with the same id; when
folded, the later line wins.

`.metaform/concepts.tsv` is the fifth file a project keeps, and it answers a different question: not what the
project knows about its objects, but **what the repository has absorbed from outside** -- a concept, the
claim this repository makes about it, and the evidence for that claim. It is append-only and retractable
in the same way. **Evidence is mandatory**: `concept add` refuses a concept with an empty `--evidence`,
because a concept with no evidence is a note, and notes are what this repository complains about losing. This repository uses it too: the `.metaform/` directory at the root is
its output (migrated from the old JSON by `scripts/migrate-ledger-to-tsv.py`).

The MCP server is registered host-side as `serverName` `metaform`, transport `stdio`, command
`metaform`, args `["mcp"]`, `cwd` at the project root, `toolCallTimeoutMs` 60000, and
`failOnStartupError` true; the model-visible names are `mcp__metaform__<tool>`. It echoes whatever
well-formed `protocolVersion` the client offers, including revisions newer than the one it was built
with, and answers `2025-11-25` only when the client sends none. `scripts/install-mcp.ps1` installs it
to a stable prefix outside the repository, checks `--version` and the handshake, and registers the
entry in each profile's patch layer, reporting the action per profile (`-NoRegister` prints the entry
instead, `-DryRun` prints the plan); the skill lives at `.dsh/skills/metaform/SKILL.md`.

## `axes`

The closed-world complaint is that the user's enumeration is a sample drawn from a set they never
stated, and a model that treats the enumeration as the set has an empty complement. `axes list` makes
the grid the current vocabulary already implies countable: the equivalence classes, the candidate
readings generated from the vocabulary (each predicate, each conjunction of two, each complement), and
three numbers -- how many of those readings agree with the evidence, how many differ from every
predicate in force (the candidates for "what you did not say"), and how many are the same distinctions
reworded.

It does not compute the axis-equivalence rate of `CHARTER.md` 6.10 over the *stated elements*, because
a stated element is an object while an axis reads a set; that layer needs concept families, which the
repository does not have yet.

## `suggest`

`suggest` is the read-only proof step for an `INSUFFICIENT` state. It enumerates every `all_of`
(arity 1 through `--max-arity`) and every `any_of` (arity `--max-arity`) over the current vocabulary,
scores each against the colliding pairs, deduplicates candidates that cover the same object mask
(keeping the first in enumeration order, with canonical sorted operands), and reports how many
candidates it `tried` and how many it `deduplicated`. Every colliding
pair shares a signature, so every Boolean recombination is constant on it (Theorem 11): `suggestions`
stays empty and `reason` says why. `next` names what can actually change the state: while some
colliding class still has a member that was never asked directly, one more question (`ask`) verifies
that member's inherited label; otherwise the state needs a predicate outside the current atoms
(`new_atom`). Asking only appends evidence and cannot remove a colliding pair (Theorem 5), so a class
whose members were all asked -- even one whose direct labels conflict -- is no longer askable. When
`next` is `ask`, the JSON also carries `ask`: one deterministic representative per colliding
equivalence class that still has an unasked member (`objects`), with `why` naming the case
(`unlabelled_class` or `unprobed_member`). `new_atom` omits the field because no question can repair
the state.

With `--emit-request`, `suggest` does not print the report. On an `INSUFFICIENT` state it prints one
ready-to-fill `metaform.propose/1` request (or writes it to `--out <path>`) and leaves the state
untouched: `protocol`, a deterministic `request_id` (`suggest-` plus a 16-hex state hash), `objects`
(the count `propose` reads) and `object_ids`, the resolved `vocabulary` (every predicate name with its
current ids), `evidence` (`pos` / `neg` as the kernel sees them under the recommended `theta`), that
`theta`, `colliding_pairs`, and `allowed` (an extensional `ids` list or a `define` over the
vocabulary). The file is valid input for `metaform propose --request <file> --response <file>` as-is,
and a response with `"candidates":[]` is a valid empty answer. Any other status prints
`{"status":"<STATUS>","request":null}` and no request, because those states do not need a predicate
outside the vocabulary.

## `explore` and `answer`

`explore` is the read-only port of conexp-clj's `explore-attributes`. The context is the predicate
table (objects `0..n`, each predicate's extension, each object's intent); the expert is the evidence:
objects with a confirmed label under the recommended `theta`. It walks the closed sets of the law
theory in a fixed `(cardinality, mask)` order, and for a premise `A` computes `B` = `A` closed under
the recorded laws and `B'` = intent of `extent(B)` over the evidence. If `B' == B` nothing follows;
if an object in `extent(B)` lacks an attribute of `B' \ B` the implication is `refuted` with that
counterexample; if no evidence object matches the premise it is left `open`. Otherwise the law
`B -> B' \ B` is recorded and the implication is reported with `by:"evidence"`. `--max-questions`
caps the number of premises examined and appends `"stopped":"budget"` when there is more to do. A
refutation cannot add the counterexample object (the state is read-only), so unlike the reference it
does not shrink later questions: only a recorded law (a Horn implication) or a recorded refutation
can. The initial theory is read from `.metaform/laws.tsv` (or `--laws <path>`; an absent file is the
empty theory), so a premise a law or a refutation already decides is not examined and is not asked
again. Output is one deterministic JSON line
`{"status":...,"questions":...,"budget":...,"implications":[...],"refuted":[...],"open":[...],"closed_sets":...}`;
`status` is the same status `state need` reports under the same `theta`, `questions` counts the
`open` entries, `closed_sets` counts the premises examined (a premise that a later law removes still
counts), and all names are sorted. A `refuted` entry carries the `by` source that decided it
(`evidence` for a refutation found in this run, `expert` for a recorded one) and, when known, the
`counterexample` object. `--out` writes the line to a file instead of stdout.
`--emit-questions <path>` writes the open questions as
`{"status":...,"questions":[{"id":n,"premise":[...],"conclusion":[...],"text":"Does every object with A also have B?"}]}`
(ids are 0-based positions in `open` order) and suppresses the normal stdout line; add `--out` when
both files are wanted. The state is never written, and neither `--out` nor `--emit-questions` may
name the state or the laws file.

The three commands form one loop: `explore --emit-questions q.json` asks, an expert writes
`{"answers":[...]}` for those ids, and `metaform answer --project . --questions q.json --answers
a.json` records the answers in `.metaform/laws.tsv` (both commands accept `--laws <path>` for a
theory file elsewhere); the next `explore` starts from that file. The file is tab-separated with the
header `source<TAB>kind<TAB>premise<TAB>conclusion<TAB>note`, one entry per line, names
comma-separated; `kind` is `law` or `refutation`, and `source` is `evidence`, `expert` or `define`.
Keeping both kinds in one file gives both commands one theory path (`--laws`), and the `kind` column
is what prevents a refutation from entering the closure as if it were a law. A `yes` appends
`expert`/`law` with the question's premise and conclusion. A `no` must carry
`{"id":k,"lacks":[names]}` where `k` is a real object that has the premise and lacks the listed
conclusion attributes, and appends `expert`/`refutation` with a note naming `k`; it never appends a
law. `explore` treats that premise as decided -- no open question and no implication for it -- and
reports the record in `refuted` with `by:"expert"`. A `define` adopts `name = op:operands` through
the same path as `state add --define` and appends `define`/`law`. Only a `define` answer writes the
state; every other answer leaves it unchanged. Replaying the same answers file does not duplicate a
line (dedupe on kind + source + premise + conclusion). An unknown id, a malformed answer, a
counterexample that does not actually refute, or a definition name that already exists exits 2 and
writes nothing.

## `coverage`

`coverage` is the read-only goal-coverage check: the same closure idea as `explore`, applied to a
table of work items against required properties. The table is TSV; the header is `item` followed by
requirement names, each later row is a work item, and a cell is `yes`, `no`, `?`, or empty (empty
means `?`); blank lines and lines starting with `#` are skipped. It validates the whole table before
printing one JSON object: `items`, `requirements`, `missing` (every unknown cell), `conflicts`,
`equivalences` (rows with identical yes/no/? patterns), and `blocking`. An implication `X -> Y` is
supported when at least two rows say `yes` to both; every row that says `yes` to X but `no` or `?`
to Y is a conflict, with `implied_by` and the reason naming the supporting rows. A requirement whose
name ends in `!` is blocking and the marker is stripped in output; a `?` in that column is listed in
`blocking` because the row cannot be read until it is answered, while a `no` is a real answer. The
default path is `.metaform/coverage.tsv` under `--project`; a positional path or `--input` overrides
it. `--format text` prints one `item: requirement: reason` line per problem. Nothing is written; a
malformed cell, duplicate requirement, unknown column, or missing header exits 2 with empty stdout.

## `check`

`check` is the repository gate that lives in the binary. `check all` runs the argument-free rules
(`prints`, `layers`, `tracked`, `members`); the rest take arguments and are run by the claims
register:

| subcommand | what it asserts |
|---|---|
| `check text <path>... --has <literal> --lacks <literal>` | a document contains what it must and none of what it must not |
| `check count <path> --lines-begin\|--contains\|--number-after <marker> [--equals N] [--same <path2> [--pattern2 <marker2>]]` | a document states the number the code states, in both languages |
| `check fresh <artefact> --than <source>` | a build product was written no earlier than its source |
| `check absent <path>...` | a stale artefact is gone |
| `check members` | every workspace member lives under `crates/` |
