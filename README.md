# metaform — query-driven representation change

A publishable theoretical work plus an executable reference implementation: when the target concept is not in the current vocabulary, increasing the search budget is bound to fail; the system must decide that "the language is insufficient" and rewrite the language using an external signal.

Paper: **English is authoritative**, `paper/en/main.tex` (11 pages) · Chinese translation, `paper/zh/main.tex` (11 pages). Both build from the tracked sources; the PDFs are build artifacts.
From now on, edit only `paper/en/main.tex`, then sync `paper/zh/main.tex`.

## Layout

| Directory | Contents |
|---|---|
| `paper/en/`, `paper/zh/` | Bilingual body text. en uses `pdflatex` twice; zh uses `xelatex` twice (ctex) |
| `research/` | Propositions (00), formal core (01), paper outline (02), literature synthesis (03), engine design (04), borrowable mechanisms (05), AI-native hypotheses (06), JEV decision layer (07), claim audit (08), long-form notes (09), unified operators (11), operator typing (12) |
| `research/lean/` | Lean 4 formalization: 13 declarations carrying Theorems 1–11, no admitted goal and no warnings; `research/lean/mechanization.md` maps theorem to declaration |
| `research/exp/` | Python experiments: synthetic domains, Lean domain, code domain, cost breakdown, data ledger `RESULTS.md` |
| `engine/` | Rust kernel + `metaform` command line (`engine/scenarios/` holds example scripts) |
| `scripts/` | Assertion gates, alignment and reference checks, Lean/Rust wrappers, bundle builder |

## Reference rules

A tracked file may only point at something the repository contains. `scripts/check-references.py`
fails on an absolute path, on a backticked path or a markdown link that git does not track (generated
artifacts and the `local.ps1` bootstrap copy excepted), on a bare mention of the untracked corpus or
conversation, and, inside either paper, on a code identifier, a file name, a URL, or a citation
without a bibliography entry. The paper reaches external work only through the bibliography at the
end, which is the usual convention; the table that maps theorems to Lean declarations lives in
`research/lean/mechanization.md` instead of the paper.

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
metaform state vote  --project . --id 0 --yes 3 --no 0
metaform state need  --project .                  # one-shot: status and recommendations directly
metaform plan   --project . --pool .metaform/out/pool.txt # coverage + purity + re-ask budget
metaform report --project .                         # generate .metaform/out/report.md
metaform score  --project . --pool cands.txt --truth truth.txt   # candidate scoring: coverage/P-R/ledger
metaform mcp                                       # MCP server (stdio) for agents to mount
metaform ops                                       # five-operator schedule comparison: fixed order vs skipping redundant steps (paper §4)
```

The ledger is **append-only**: retracting means appending a `retracted` line with the same id; when
folded, the later line wins.
This repository uses it too: the `.metaform/` directory at the root is its output (migrated from the old JSON by `scripts/migrate-ledger-to-tsv.py`).

## Reproducing the three-part setup

```powershell
pwsh -File scripts/check-all.ps1                     # six assertion gates (Lean/Rust/references/falsification/bilingual/numbers), about 40 s
pwsh -File scripts/check-lean.ps1                    # Lean only (any warning fails)
pwsh -File scripts/check-rust.ps1                    # Rust only (pass count must == 63)
pwsh -File scripts/selftest-gates.ps1                # negative self-test: make each gate go red once
python scripts/align-papers.py                       # bilingual structure/number alignment + mechanization map ↔ Lean source
python scripts/check-references.py                   # no path, file name or link that the repository does not contain
python research/exp/ledger.py list --all             # counterexample ledger (add/amend/retract, suspend, with evidence and history)
python research/exp/ledger.py show L2               # inspect one entry: source, answer, votes, change history
python research/exp/ledger.py corroborate --apply    # merge weak evidence for the same object via noisy-OR
python research/exp/ledger.py conflicts              # direction conflicts + re-ask queue (sorted by gap, includes json)
pwsh -File scripts/make-bundle.ps1                   # build the submission bundle (read back and verify every entry) → dist/submission-<date>.zip
python research/exp/verify_claims.py                 # paper-number regression (tolerance 0.06)
python research/exp/theory_audit.py --engine engine/target/debug/metaform.exe   # theorem falsification + engine consistency
python research/exp/domain_a.py --n 300 --objects 12 --attrs 8 --k 2
python research/exp/domain_b.py                      # Lean as oracle, about 3 s
python research/exp/domain_c.py                      # real code invariants
python research/exp/domain_cost.py                   # cost breakdown
python research/exp/domain_more_data.py              # empirical version of Theorem 5
python research/exp/greedy_cover.py                  # greedy cover vs optimal vs random
engine\target\debug\metaform.exe ops                 # five operators' schedule comparison + 200 random self-checks
python research/exp/noisy_oracle.py --attrs 4 --objects 12 --examples 8   # noisy oracle
python research/exp/proposer.py --case domain_c --emit                    # see the request sent to the model
python research/exp/proposer.py --case domain_c --responses research/exp/responses_domain_c.json \
    --pool-out pool.txt --scenario-out case.demo                          # generate a metaform scenario and candidate pool
```

The scripts take the interpreter from `METAFORM_PYTHON` (or from `python` on PATH). If `python` on
PATH is a Store stub rather than a working interpreter, set the variable once per shell:

```powershell
$env:METAFORM_PYTHON = '<full path to python.exe>'
& $env:METAFORM_PYTHON research\exp\domain_a.py
```

A fresh clone has no machine paths, so `check-all.ps1`, `check-lean.ps1` and `selftest-gates.ps1`
all dot-source `scripts/local.ps1` when it exists. Copy the tracked template and fill it in once:

```powershell
Copy-Item scripts\local.example.ps1 scripts\local.ps1
# then set METAFORM_PYTHON, METAFORM_LEAN_PROJECT (and optionally MATHLIB_CACHE_DIR)
```

`local.ps1` is gitignored; it carries machine facts, not repository content.

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

## Honest boundaries

- The core mathematics is folklore from FCA/ILP ("definable = union of equivalence classes"). The contribution is to use it as a **necessity criterion**
  and to supply the typing of multiple operators (paper §4: monotone, idempotent, the only one that decreases, recombination does not break collisions), with triggers, baselines, and negative results following.
- All experiments are on synthetic domains and one small code domain, with **no real LLM proposer** in the loop; the paper scopes its claims accordingly and does not overclaim.
- The counting form `2^m` of T4 is formalized as `capacity_bound_strong`; the weak form is kept as a lemma.

## Provenance

- The problem was posed in one long research conversation. That conversation is an input, not a
  deliverable, so it is not tracked here.
- `research/03-synthesis.md`: paper-by-paper comparison of 36 works, confirming the gap of
  "query failure -> rewrite the language".

