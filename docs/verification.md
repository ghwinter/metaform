# What is checked, and how

Two gates, because two things are being built. `scripts/check-all.ps1` checks the program: the
crates, the experiments, the claims about them. `scripts/check-paper.ps1` checks the paper and the
formalization: Lean, the bilingual alignment, and whether the built PDF is newer than its source.

They are separate on purpose. A compiled formalization and a typeset PDF are not part of building the
program, and making the program's gate depend on them meant that a Lean toolchain or a TeX
distribution could block a commit about a Rust function. The formalization is still checked -- on the
run that is about the formalization.

## `check-all.ps1`

```powershell
pwsh -File scripts/check-all.ps1
```

| check | what it asserts |
|---|---|
| `rust` | the pass count must == 198, 0 failed, 0 ignored |
| `kernel` | the measure's own tests, against its own manifest (12) |
| `propose` | the operator protocol's own tests (25) |
| `store` | the five on-disk artefacts' own tests (18) |
| `vsa` | the hyperdimensional component's own tests (19) |
| `ilp` | the inductive learner's own tests (17) |
| `refs` | no path, file name or link that the repository does not contain |
| `components` | the proposal components build, and three proposers are judged by one validator |
| `claims` | every claim about the program still has a command that supports it |
| `backlog` | no declared-but-undone item is marked done without a claim behind it |
| `secrets` | tracked files and tmp carry no API-key-shaped strings |
| `checks` | `metaform check all`: prints, layers, tracked documents, workspace members |
| `theory` | the theorems still hold against the engine, and the engine against them |
| `experiments` | the paper's numbers and both loop scripts still reproduce |

The per-crate counts are checked because `cargo test` returns zero when a test is `#[ignore]`d or an
entire module is `cfg`'d out; pinning the number is what makes that visible.

## `check-paper.ps1`

```powershell
pwsh -File scripts/check-paper.ps1
```

| check | what it asserts |
|---|---|
| `lean` | `proofs/QueryDriven.lean` compiles with no `sorry` and no warnings |
| `align` | the two papers keep the same structure and the same numbers; the mechanization map and the Lean source name the same declarations |
| `pdf` | the built English PDF is not older than its source |
| `claims` | the `paper` rows of the claims register |

## The claims register

`experiments/claims.tsv` lists sentences the repository asks a reader to believe, each with a command
whose output would refute it. `scripts/check-claims.py` runs them: `PASS` means the prose still holds,
`FAIL` is a gate failure, and `NEED` lists the claims that have no executable check -- the honest list
of what is asserted but not verified. Adding a claim means adding its command in the same commit.

Each row carries a `scope`. `code` rows are run by `check-all`, `paper` rows by `check-paper`; a row
is never skipped by both. The commands are part of the product where they can be: `metaform check
text`, `check count`, `check fresh`, `check absent` and `check members` read the repository, so a
claim about a document does not need a shell, and `python` appears only where the work is an
experiment.

## Reference rules

A tracked file may only point at something the repository contains. `scripts/check-references.py`
fails on an absolute path, on a backticked path or a markdown link that nothing answers to (generated
artifacts, the `local.ps1` bootstrap copy, and the three untracked working directories `scripts/`,
`experiments/`, `.dsh/` excepted), on a bare mention of the untracked corpus or conversation, and,
inside either paper, on a code identifier, a file name, a URL, or a citation without a bibliography
entry. The paper reaches external work only through the bibliography at the end, which is the usual
convention; the table that maps theorems to Lean declarations lives in `proofs/mechanization.md`
instead of the paper.

The tree is default-deny: `.gitignore` starts with `*` and then allows whole directories
(`!/crates/**`, `!/docs/**`) and the few root files that belong to the repository. A new source file
inside an allowed directory needs no edit; a new top-level directory does. The same gate fails if a
tracked file lies outside every allowed rule, or if an allowed rule matches nothing, so a dropped note
or a build product cannot enter the history by accident.

## Metered runs and secret scanning

`scripts/metered-run.ps1` runs one headless task with `--json`, sums the step usage, and writes the
metrics JSON; it takes the key from the variable named by `-KeyEnv` or from an external `-KeyFile`,
refuses a key file inside the repository, sets a file-sourced key only in the child environment, and
keeps raw stdout under tmp only when `-Raw` is given. `scripts/check-secrets.py` scans the tracked
files and tmp for key-shaped strings and has a `--self-test`; `check-all` runs it as the `secrets`
gate.

## Commits

Messages follow Conventional Commits: a type (`feat`, `fix`, `docs`, `test`, `refactor`, `chore`), an
optional scope, and an imperative summary. The body says what changed and why, in the same voice as
the code comments: no machine paths, no names of untracked material.

## Reproducing the measurements

```powershell
python experiments/verify_claims.py                 # paper-number regression (tolerance 0.06)
python experiments/theory_audit.py --engine target/debug/metaform.exe   # theorem falsification + engine consistency
python experiments/ledger.py list --all             # counterexample ledger (add/amend/retract, suspend, with evidence and history)
python experiments/ledger.py show L2               # inspect one entry: source, answer, votes, change history
python experiments/ledger.py corroborate --apply    # merge weak evidence for the same object via noisy-OR
python experiments/ledger.py conflicts              # direction conflicts + re-ask queue (sorted by gap, includes json)
python experiments/domain_a.py --n 300 --objects 12 --attrs 8 --k 2
python experiments/domain_b.py                      # Lean as oracle, about 3 s
python experiments/domain_c.py                      # real code invariants
python experiments/domain_cost.py                   # cost breakdown
python experiments/domain_more_data.py              # empirical version of Theorem 5
python experiments/greedy_cover.py                  # greedy cover vs optimal vs random
python experiments/noisy_oracle.py --attrs 4 --objects 12 --examples 8   # noisy oracle
target\debug\metaform.exe ops                 # five operators' schedule comparison + 200 random self-checks
target\debug\metaform.exe crates\metaform\scenarios\review.demo   # the necessity criterion applied to a review policy
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
The Lean requirement is declared but not required by `check-all`: it is what `check-paper` needs.

## Packaging

```powershell
pwsh -File scripts/make-bundle.ps1   # build the submission bundle (read back and verify every entry) → dist/submission-<date>.zip
```
