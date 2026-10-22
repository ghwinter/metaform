# metaform — query-driven representation change

A publishable theoretical work plus an executable reference implementation: when the target concept is not in the current vocabulary, increasing the search budget is bound to fail; the system must decide that "the language is insufficient" and rewrite the language using an external signal.

Paper: **English is authoritative**, `paper/en/main.tex` (11 pages) · Chinese translation, `paper/zh/main.tex` (11 pages). Both build from the tracked sources; the PDFs are build artifacts.
From now on, edit only `paper/en/main.tex`, then sync `paper/zh/main.tex`.

## Layout

| Directory | Contents |
|---|---|
| `paper/en/`, `paper/zh/` | Bilingual body text. en uses `pdflatex` twice; zh uses `xelatex` twice (ctex) |
| `proofs/` | Lean 4 formalization: 13 declarations carrying Theorems 1–11, no admitted goal and no warnings; `proofs/mechanization.md` maps theorem to declaration |
| `experiments/` | Python experiments: synthetic domains, Lean domain, code domain, cost breakdown, data ledger `RESULTS.md` |
| `engine/` | Rust kernel + `metaform` command line (`engine/scenarios/` holds example scripts) |
| `scripts/` | Assertion gates, alignment and reference checks, Lean/Rust wrappers, bundle builder |

The charter, `CHARTER.md` (Chinese copy `CHARTER.zh.md`), states the problem, the
thirteen claims each with a falsifier, the six positions of the runtime, the ten laws, and the route. If
this README and the charter disagree on a design principle, the charter wins; if either disagrees with the
code or the gates, the code and the gates win.

## Reference rules
A tracked file may only point at something the repository contains. `scripts/check-references.py`
fails on an absolute path, on a backticked path or a markdown link that git does not track (generated
artifacts and the `local.ps1` bootstrap copy excepted), on a bare mention of the untracked corpus or
conversation, and, inside either paper, on a code identifier, a file name, a URL, or a citation
without a bibliography entry. The paper reaches external work only through the bibliography at the
end, which is the usual convention; the table that maps theorems to Lean declarations lives in
`proofs/mechanization.md` instead of the paper.

The tree is default-deny: `.gitignore` starts with `*` and then allows whole directories (`!/engine/**`)
and the few root files that belong to the repository. A new source file inside an allowed directory
needs no edit; a new top-level directory does. The same gate fails if a tracked file lies outside every
allowed rule, or if an allowed rule matches nothing, so a dropped note or a build product cannot enter
the history by accident.

## Metered runs and secret scanning

`scripts/metered-run.ps1` runs one headless task with `--json`, sums the step usage, and writes the
metrics JSON; it takes the key from the variable named by `-KeyEnv` or from an external `-KeyFile`,
refuses a key file inside the repository, sets a file-sourced key only in the child environment, and
keeps raw stdout under tmp only when `-Raw` is given. `scripts/check-secrets.py` scans the tracked
files and tmp for key-shaped strings and has a `--self-test`; `scripts/check-all.ps1` runs it as the
`secrets` gate.

## `metaform`: lay out the current scenario

```powershell
cd engine
cargo run --quiet -- scenarios\setcover.demo
```

Commands: `objects N` / `add <name> <id...>` / `pos <id...>` / `neg <id...>` / `vote <id> <yes> <no>` /
`ask` / `status` / `explain` / `score <name> <id...>` / `capacity` / `need [theta]` / `batch k` /
`gen [pool.txt]` (generate a metarule-style candidate pool from the current vocabulary) / `cover <pool.txt>` (greedily cover all colliding pairs).

`need` outputs a machine-readable status (including `action`: `PROPOSE` / `ASK` / `STOP`), the vote-gap
threshold `theta` (defaults to `theta_rec`), `epsilon_hat` estimated from the vote counts, the
recommended `theta_rec`, and `pending` objects that need more votes.
Under a noisy oracle it suppresses false triggers (see `RESULTS.md` §3.11: at ε=0.1, θ=1 succeeds only 25% of the time, while θ=3 succeeds 98%).

### Project directory and bootstrapping

`metaform` writes to exactly one subdirectory, `.metaform/`:

```powershell
metaform init   --project .                      # create .metaform/{config.toml,ledger.tsv,logs,out}
metaform ledger add     --project . --case 3 --by demo --reason "first piece of evidence"
metaform ledger list    --project . --all        # TSV, folded by id to the latest state
metaform ledger retract --project . --id L2 --reason "oracle answered wrong"
metaform ledger cases   --project .              # confirmed objects with no conflicts
metaform ledger conflicts --project .            # direction conflicts: {"conflicts":[...]}
metaform state init  --project . --n 4            # persist the scenario to .metaform/state.json
metaform state add   --project . --name p --define all-of:a,b  # store p = a AND b as a definition
metaform state vote  --project . --id 0 --yes 3 --no 0
metaform state need  --project .                  # one-shot: status and recommendations directly
metaform apply  --project . --request request.json --response response.json # validate + adopt accepted candidates with one state write
metaform check-predictions --project .            # read-only: score recorded held-out predictions against the state's strong labels
metaform suggest    --project . [--max-arity 2] [--max 20] [--emit-request [--out <path>]] # read-only: Boolean candidates tried, and why none separates
metaform explore    --project . [--max-questions 1000] [--laws <path>] [--emit-questions <path>] [--out <path>] # read-only: implications confirmed by evidence, refuted with a counterexample, or open
metaform answer     --project . --questions <q.json> --answers <a.json> [--laws <path>] # record expert answers as laws (and adopt a define), so the next explore starts from them
metaform coverage   [--project .] [--format text] [<table.tsv>] # read-only: missing cells, implication conflicts, duplicate rows, blocking gaps
metaform plan   --project . --pool .metaform/out/pool.txt # coverage + purity + re-ask budget
metaform report --project .                         # generate .metaform/out/report.md
metaform score  --project . --pool cands.txt --truth truth.txt   # candidate scoring: coverage/P-R/ledger
metaform mcp                                       # MCP server (stdio) for agents to mount
metaform metrics verdict <rows.tsv>                # precision on answered, unknown rate, selective accuracy, AURC
metaform metrics bets <predictions.tsv>            # share of recorded bets placed where two rules disagree
metaform metrics readings --vocab <tsv> --readings <tsv>  # complement coverage and axis equivalence rate
metaform ops                                       # five-operator schedule comparison: fixed order vs skipping redundant steps (paper §4)
```

The MCP server is registered host-side as `serverName` `metaform`, transport `stdio`, command `metaform`, args `["mcp"]`, `cwd` at the project root, `toolCallTimeoutMs` 60000, and `failOnStartupError` true; the model-visible names are `mcp__metaform__<tool>`. It echoes whatever well-formed `protocolVersion` the client offers, including revisions newer than the one it was built with, and answers `2055-11-25` only when the client sends none. `scripts/install-mcp.ps1` installs it to a stable prefix outside the repository, checks `--version` and the handshake, and registers the entry in each profile's patch layer, reporting the action per profile (`-NoRegister` prints the entry instead, `-DryRun` prints the plan); the skill lives at `.dsh/skills/metaform/SKILL.md`.

The ledger is **append-only**: retracting means appending a `retracted` line with the same id; when
folded, the later line wins.
This repository uses it too: the `.metaform/` directory at the root is its output (migrated from the old JSON by `scripts/migrate-ledger-to-tsv.py`).

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

## LLM adapter

The engine never calls a model itself; it validates what a model returns. `metaform propose` reads a
request file and a response file under protocol `metaform.propose/1` and reports what it accepts.
Candidates are extensional (a name plus the object ids it covers) or defined (`all_of` / `any_of` over
names that already exist in the vocabulary). The validator rejects ids outside the object range, constant
or empty extensions, names that repeat the vocabulary or an earlier candidate, duplicate ids, and anything
arriving under the wrong protocol or request id; a model may also decline,
which is recorded as `refused` rather than retried. Validation is pure: it writes no file and changes no
state, so a proposal cannot overwrite evidence; `apply` is the explicit adoption step.
Adopting an accepted definition persistently uses `metaform state add --project . --name p --define
all-of:a,b` (or `any-of:a,b`); `--ids` keeps the extensional form. The state file stores the definition
next to its extension, and every load recomputes that extension from the operands and the current object
count, so a project that grows past the object set it was saved with keeps a correct predicate. A missing
operand or a definition cycle is a clear error rather than a silently dropped predicate; files written
before definitions existed load unchanged and stay extensional.

`metaform apply --project . --request <request.json> --response <response.json>` collapses proposal and
adoption into one command. It runs the same validator, adopts every accepted candidate exactly as
`state add` would (an extensional candidate by its ids, a defined candidate by its definition, so the
state recomputes the extension), and writes `.metaform/state.json` once at the end. A candidate whose
name already exists in the state is skipped and reported (`name_exists`); candidates the validator
rejected are reported in `skipped` with the validator's reason, and the rest of the response is still
applied. A whole-response rejection (wrong protocol or request id, malformed JSON) exits non-zero and
leaves the file unchanged, as does a definition that cannot resolve, because every adoption happens on
a copy before the single write. The command prints one deterministic JSON line:
`{"applied":n,"skipped":[{"name":...,"reason":...}],"before":{"status":...,"colliding":n},"after":{"status":...,"colliding":n},"request_id":"..."}`.
`before` and `after` use the same recommended-`theta` status and colliding-pair count as `suggest` and
`state need`, so the one-command path cannot disagree with the read-only report. The request and
response files are only read.

L11 makes the held-out prediction part of validation. A request that carries `object_ids` names its
held-out test: every id in that list that is absent from `evidence` is *unmeasured*. On such a request
a candidate must carry `"predict":{"id":<object id>,"label":"pos"|"neg"}`, or it is rejected with
`missing_prediction`; the prediction is echoed unchanged on acceptance. When present it is always
checked: an id outside `0..objects` is `prediction_out_of_range`, an id already in `evidence` is
`predicted_measured_object` (it is not a held-out test), and a label other than `pos` / `neg` is
`prediction_bad_label`. A request without `object_ids` -- the pre-L11 shape -- declares no unmeasured
object, so its candidates keep the old optional field and the old verdicts. A prediction only counts
as a bet when another rule could have made a different one, so a candidate may name the rule it is
competing with: `"rival":{"ids":[...]}"` or `"rival":{"define":{"all_of"|"any_of":[...]}}`. The rival
is parsed by the candidate's own rules and its extension is recomputed from the request vocabulary;
the prediction must then name an object where the two extensions disagree, or the candidate is
rejected with `predict_not_discriminating`. A rival without a prediction is `missing_prediction`,
because there is nothing left to compare. A candidate without `rival` behaves exactly as before and
keeps its old verdict, so a coherent but false rule can no longer pass on a prediction that every
rule in its neighbourhood would make too.

`apply` records every adopted prediction in `.metaform/predictions.tsv`: one header line plus one
tab-separated row per prediction (`request_id`, candidate name, object id, predicted label, the
candidate's extension as a comma-separated id list, and the rival's extension, empty when the
candidate named none). Rows dedupe on `(request_id, name, id)`, so replaying a response does not
duplicate a commitment, and the same input writes the same bytes. The rival column is last, so a
record written before it existed still loads: a five-column header and four- or five-field rows are
read with an empty rival.
`metaform check-predictions --project .` is read-only: it takes the same recommended-`theta` strong
labels the kernel uses and compares each recorded prediction with the object's actual label, printing
`{"checked":n,"correct":n,"wrong":[{"name":...,"id":...,"predicted":...,"actual":...}],"open":n,"discriminating":n}`;
`open` counts predictions whose object has no strong label yet, and `discriminating` counts the
checked predictions whose non-empty rival disagreed with the candidate on the predicted object.

Under the cheap-proposal assumption this is the division that matters: the model supplies many
candidates per call, the engine pays for the queries.

## Components

The kernel is not the whole tool. The proposers sit behind the operator protocol, each algorithm in
its own crate, and none of them may change the state:

| crate | what it proposes | how |
|---|---|---|
| `crates/vsa` | predicates whose vector similarity to a positive/negative contrast is high | MBAT binding and bundling, `retrieve` |
| `crates/sme` | predicates inferred by mapping one relational system onto another | structure mapping, `analogize` |
| `crates/ilp` | clauses whose extension covers the positives and no negative | relational learning, `ilp_demo` |
| `crates/runtime` | nothing: it is the kernel | protocol, validation, controller, record, `Operator` |
| `crates/components` | the concrete operators and the CLI | `VsaRetriever`, `SmeAnalogizer`, `ProbeLabels`, `metaform-runtime` |

A component proposes, the kernel judges:

```powershell
target\debug\metaform-runtime.exe retrieve  --state crates/runtime/fixtures/state.txt --out response.json
target\debug\metaform-runtime.exe analogize --state crates/runtime/fixtures/state.txt --systems crates/runtime/fixtures/systems.txt --out response.json
target\debug\metaform.exe propose --request crates/runtime/fixtures/request.json --response response.json
```

The controller does the adopting, and records what it did:

```powershell
target\debug\metaform-runtime.exe run --state crates\runtime\fixtures\state.txt --record runs.tsv
```

It asks every applicable operator, keeps what the validator accepts, adopts the proposal that
separates the most colliding pairs (cheapest on a tie), stops when nothing separates anything, and
appends one line per step — state fingerprint, operator, proposal, pairs separated, adopted, cost,
status before and after, and the state class (objects, vocabulary size, labelled examples, colliding
pairs) before the step.

One record line is written per **offer**, not per step, so a proposal that lost is visible too.
The record is then queryable: `metaform-runtime experience --record runs.tsv --state <file>` prints
the tallies per class and says which operator has adopted most often in the class of that state;
`run --use-record runs.tsv` puts that operator first. With `--probe` the controller also reports a
`conflict:` line -- and records it -- when the adopted proposal cannot separate every pair and the
probe has named a label it had to disbelieve. Order is the only thing experience changes —
every operator is still asked, and the controller still decides by pairs separated.

Anything that is not a Rust component — a model, a script, a person with an editor — can compete by
writing a proposals file and passing `--proposals`:

```text
# label   name      ids
k2_0      a4        0 5 10 11
```

The third line above is the only one that can accept anything, and accepting is still not adopting:
`experiments/proposer_arms.py` gives one state to three proposers (random, retrieval, and the
proposals a language model wrote) and reports what each found, judged by the same kernel.

## Claims about the repository

`experiments/claims.tsv` lists sentences the repository asks a reader to believe, each with a command
whose output would refute it. `scripts/check-claims.py` runs them: `PASS` means the prose still holds,
`FAIL` is a gate failure, and `NEED` lists the claims that have no executable check — the honest list
of what is asserted but not verified. Adding a claim means adding its command in the same commit.

## Commits

Messages follow Conventional Commits: a type (`feat`, `fix`, `docs`, `test`, `refactor`, `chore`), an
optional scope, and an imperative summary. The body says what changed and why, in the same voice as the
code comments: no machine paths, no names of untracked material.

## Reproducing the three-part setup

```powershell
pwsh -File scripts/check-all.ps1                     # eleven gates (lean/rust/vsa/align/refs/components/claims/secrets/checks/theory/experiments), about 40 s
pwsh -File scripts/check-lean.ps1                    # Lean only (any warning fails)
pwsh -File scripts/check-rust.ps1                    # Rust only (pass count must == 205)
pwsh -File scripts/selftest-gates.ps1                # negative self-test: make each gate go red once
python scripts/align-papers.py                       # bilingual structure/number alignment + mechanization map ↔ Lean source
python scripts/check-references.py                   # no path, file name or link that the repository does not contain
python experiments/ledger.py list --all             # counterexample ledger (add/amend/retract, suspend, with evidence and history)
python experiments/ledger.py show L2               # inspect one entry: source, answer, votes, change history
python experiments/ledger.py corroborate --apply    # merge weak evidence for the same object via noisy-OR
python experiments/ledger.py conflicts              # direction conflicts + re-ask queue (sorted by gap, includes json)
pwsh -File scripts/make-bundle.ps1                   # build the submission bundle (read back and verify every entry) → dist/submission-<date>.zip
python experiments/verify_claims.py                 # paper-number regression (tolerance 0.06)
python experiments/theory_audit.py --engine target/debug/metaform.exe   # theorem falsification + engine consistency
python experiments/domain_a.py --n 300 --objects 12 --attrs 8 --k 2
python experiments/domain_b.py                      # Lean as oracle, about 3 s
python experiments/domain_c.py                      # real code invariants
python experiments/domain_cost.py                   # cost breakdown
python experiments/domain_more_data.py              # empirical version of Theorem 5
python experiments/greedy_cover.py                  # greedy cover vs optimal vs random
target\debug\metaform.exe ops                 # five operators' schedule comparison + 200 random self-checks
target\debug\metaform.exe scenarios\review.demo   # the necessity criterion applied to a review policy
python experiments/noisy_oracle.py --attrs 4 --objects 12 --examples 8   # noisy oracle
python experiments/proposer.py --case domain_c --emit                    # see the request sent to the model
python experiments/proposer.py --case domain_c --responses experiments/responses_domain_c.json \
    --pool-out pool.txt --scenario-out case.demo                          # generate a metaform scenario and candidate pool
```

The scripts take the interpreter from `METAFORM_PYTHON` (or from `python` on PATH). If `python` on
PATH is a Store stub rather than a working interpreter, set the variable once per shell:

```powershell
$env:METAFORM_PYTHON = '<full path to python.exe>'
& $env:METAFORM_PYTHON experiments\domain_a.py
```

A fresh clone has no machine paths, so the repository declares what it needs instead of shipping one
machine's answer: `environment.json` names each required capability, the environment variable that
supplies it, and a minimum version. Check the machine, then let the repository write the file it
sources:

```powershell
pwsh -File scripts/bootstrap.ps1          # what is present, what is missing, how to supply it
pwsh -File scripts/bootstrap.ps1 -Print   # the same, as instructions for an agent
pwsh -File scripts/bootstrap.ps1 -Write   # write scripts/local.ps1 from the values it observed
```

`local.ps1` is generated and gitignored; it carries machine facts. Versions are a different matter:
anything that pins what the results are (Lean version, Mathlib revision, the Rust toolchain,
`Cargo.lock`) stays tracked, because two clones that disagree there can no longer check each other.

## Conclusions (checkable)

1. **T2 (necessity)**: if a positive and a negative example share the same signature class, then no consistent concept exists in the current vocabulary. Reproduced 300/300 in the synthetic domain.
2. **T5 (irreparable)**: more examples do not fix an insufficient vocabulary, they only expose it. P(insufficient) goes from 2.5% to 98.7% as the number of examples grows.
3. **Question selection is a second-order term**: information gain ties with "ask a random question that still carries information"; even with a prior it leads by only 9–13%.
4. **Cost is determined by proposal quality**: total cost spans 45× and is almost entirely determined by `r/q`; stopping rules differ by ±6%.
   `verify_claims.py` now checks three things: the relative error of `r/q+7.6` against the four measured points is <4%; the word
   `exactly` must no longer appear in the paper; and it actually runs `metaform ops` to check `3.11/3.50/3.60` and the probe's `1/5/14`, then parses
   the same numbers out of the README as an independent third-party comparison (paper, README, and program must all agree).
5. **A wrong predicate gives zero help**: a predicate that takes the same value on a colliding pair does not change any signature.
6. **T8–T11 (operator typing)**: the five step types share a single measure, the number of live hypotheses. Generation only raises it, closure is idempotent,
   only evidence gathering strictly lowers it, and Boolean recombination inside the vocabulary cannot break a collision; a representation's
   warrant requires the new atom to be outside the vocabulary and the candidates it admits to be definable under the new vocabulary (otherwise
   it merely records what was unexpressed as expressed). Coverage (T7) serves only as a scheduling score. Representation change and evidence gathering must therefore alternate.
7. **The entire benefit of control equals the cost of redundant steps**: in the three `metaform ops` scenarios, the difference between "execute everything" and "skip redundant steps" is
   always exactly the cost of the skipped steps; 200 random schedule self-checks produced 0 violations.
8. **Under the same budget, "choosing the question" is worth about 0.4 queries**: with eight live candidates and unit cost, stepwise selection takes 3.11 queries, random order
   3.50, and fixed order 3.60 (`metaform ops` `selection` field, 200 runs). Selection is a second-order term, not a first-order one — consistent with the negative result in paper §5.
9. **Greedy composition still holds with two atoms**: in scenarios that need 2 atoms, greedy uses 2.00 atoms with success rate 1.00;
   random order 2.87 / 0.13; fixed order (whole-class atoms first) 3.00 / 0.00 (`compose` field, 200 runs).
10. **On a real domain, ordering can substitute for a good proposer**: after sorting domain C's four candidates (three decoys + one correct) by coverage,
    one proposal costs 28, the same as "propose it right the first time"; file order takes 4 proposals and 88; random order 2.46 and 57.2
    (`selection_domain_c.py`, 200 random seeds).
11. **No collision ≠ correct**: in the domain C scenario that needs two predicates, label inheritance within a class lets the loop stop with a wrong extension —
    greedy makes 1 proposal at cost 28, but the extension does not equal the target. After probing one unasked member of each multi-member class, the state
    becomes insufficient again, and two proposals restore correctness (cost 51; file order 111); the single-atom case costs only 3 extra queries (28 → 31).
12. **Kernel termination now requires "no inherited labels"**: `run_selected` stops only when `Φ=1` **and** `unprobed` is empty;
    otherwise it schedules `Op::Probe` first. A probe that falsifies → the state falls back to insufficient; a consistent probe → the unverified set shrinks;
    when no probe operator is available it stops at `Φ=1`, but `unprobed` exposes the risk to the caller.
13. **This is now visible in tool output and checked by a gate**: the `probe` field of `metaform ops` — early stop after 1 query,
    `Φ=1`, wrong extension; the verified version takes 5 queries, costs 14, and gets the extension right. `theory_audit.py` asserts both; changing the JSON to
    `early_correct: true` turns `check-all` red (the negative self-test is in `selftest-gates.ps1`).
14. **The CLI also asks for verification first**: `objects 3; add p 0; vote 0 3 0; vote 1 3 0; need` outputs
    `"action":"VERIFY"` and `"unverified":[2]` — the label of class `{1,2}` is inherited from 1, and 2 was never asked directly.
15. **One end-to-end run works**: `engine/scenarios/verify.demo` (6 test files, deciding "is this an integration test") goes through
    `IDENTIFIED/VERIFY → INSUFFICIENT/PROPOSE → UNDETERMINED/ASK → IDENTIFIED/STOP`.
    `e2e_verify.py` freezes this sequence and `unverified=[4,5]` into `check-all`.
    While there, it also fixed the unstable order in `unprobed()` (`classes()` goes through a `HashMap`).
16. **The report shows that probing changed the verdict**: before probing, `report` gives `VERIFY` + `## To verify` (4, 5); after probing 4 it gives
    `INSUFFICIENT` + collision `(0,4)`. The project mode of `e2e_verify.py` also freezes this into
    `check-all` (build a project in a temp directory, write the ledger, read the report twice).
17. **The tool records its own evidence**: `state vote --record --by probe --reason "..."` writes this vote into
    `ledger.tsv` (ids `L1`…), and `report` gains a `## Recent evidence` section that cites it; the E2E now uses this path
    instead of hand-writing `ledger add`.

18. **Cheap proposals do not remove the evidence floor**: as the proposal cost falls toward zero the cost law tends to the 7.6 query rounds, so cheap thinking buys a better vocabulary, not fewer questions; queries spent under an insufficient vocabulary are unrecoverable (T5). A portfolio of candidate vocabularies is paid for in the very currency it is meant to save.

19. **Cheap proposals buy the vocabulary, not the queries**: with `k` candidate vocabularies of which exactly one is sufficient, a baseline that draws one member succeeds `1/k` (0.250 at `k=5`) while the portfolio succeeds 0.955, paying 1.53 queries to eliminate each wrong candidate (`metaform portfolio`, 200 trials). The portfolio never decides a wrong extension; conflict is its only failure and rises from 0.025 at `k=3` to 0.110 at `k=10`, so success peaks at `k=3` (0.975) and falls back at `k=10` (0.890).

20. **A review policy fits its taxonomy except in one place**: the pre-revision manuscript was given, blind, to two reviewers with the same checklist. They agreed on 14 of 16 findings (Cohen's kappa 0.67); the author's own decisions agree with them on 11 and 9 of the 16 (kappa below zero), so those labels are not a stable policy and are not used here. On the 14 agreed findings, the taxonomy (abstract, numbers, cross-references, titles, captions, terminology, typography, notation, voice) leaves the state exhausted: the voice findings split between "proved" (rejected — a probe refutes deductively) and tense or register (adopted), giving colliding pairs (11,13) and (12,13). One predicate, claim strength, is enough to identify the rule. `engine/scenarios/review.demo` reproduces both states.
## Honest boundaries

- The core mathematics is folklore from FCA/ILP ("definable = union of equivalence classes"). The contribution is to use it as a **necessity criterion**
  and to supply the typing of multiple operators (paper §4: monotone, idempotent, the only one that decreases, recombination does not break collisions), with triggers, baselines, and negative results following.
- All experiments are on synthetic domains and one small code domain, with **no real LLM proposer** in the loop; the paper scopes its claims accordingly and does not overclaim.
- The counting form `2^m` of T4 is formalized as `capacity_bound_strong`; the weak form is kept as a lemma.

## Remaining route

The release stops at a validated boundary, not at a runtime. What is left, in order:

1. **Lean inside the repository.** The mechanization check borrows a Lake project whose path is
   machine-local, so a clone can read the proofs but cannot recompile them. Track the lakefile and the
   toolchain (Mathlib revision included), generate the manifest, and let `check-lean` run `lake env lean`
   in place. Acceptance: a clone with only `lake update` reports the same 13 declarations, no warnings.
2. **A real model behind the protocol.** The propose boundary is pure and validated, but every response
   so far is fabricated by a mock. An adapter changes no engine code, only the file that fills the
   response. Acceptance: the same loop against a model, with accepted/rejected counts and queries
   recorded, and a budget-matched comparison of a direct answer against the loop.
3. **Types for `decompose` and `analogize`.** The five classes type evidence and representation. Goal
   decomposition changes the goal and structure mapping changes the relational structure; neither has a
   stated effect on the measure, so a scheduler cannot reason about them. Acceptance: a proposition per
   operator, a Rust test, and an extended operator table that leaves Theorems 8--11 intact.
4. **A relational necessity criterion.** Done for the tree-shaped fragment: colour refinement decides
   whether any pattern separates two objects, the witness is minimised to a readable clause, and a
   bisimilar pair reports "no separating pattern". Open: general conjunctive queries (a homomorphism
   test, NP-hard in general) and feeding a learned relation into the kernel's object set.
5. **Memory that cannot ossify.** The ledger records evidence, not strategy. An operator-experience
   table restricted to ledger-verified entries, and an abstraction library expressed as vocabulary
   growth across tasks, are the two pieces a metareasoning runtime needs. Acceptance: two task families, a measurable
   drop in proposals per task, and no strategy adopted without a verified entry.

## License

The code, the proofs, and the gates are released under MIT OR Apache-2.0, at your option; the two
texts are `LICENSE-MIT` and `LICENSE-APACHE`. The paper is released under the Creative Commons
Attribution 4.0 International license (CC BY 4.0).

## Provenance

- The problem was posed in one long research conversation. That conversation is an input, not a
  deliverable, so it is not tracked here.
- `CHARTER.md`: the stances and prohibitions distilled from that conversation; the longer notes behind
  them are kept outside the history until the engine is stable.
- The paper-by-paper comparison of 36 works that confirmed the gap of "query failure -> rewrite the
  language" is one of those notes; the paper states the conclusion and its citations.

