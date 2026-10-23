# metaform — query-driven representation change

A publishable theoretical work plus an executable reference implementation: when the target concept is not in the current vocabulary, increasing the search budget is bound to fail; the system must decide that "the language is insufficient" and rewrite the language using an external signal.

Paper: **English is authoritative**, `paper/en/main.tex` (11 pages) · Chinese translation, `paper/zh/main.tex` (11 pages). Both build from the tracked sources; the PDFs are build artifacts.
From now on, edit only `paper/en/main.tex`, then sync `paper/zh/main.tex`.

## Build and run

```powershell
cargo build --workspace --bins
target\debug\metaform.exe crates\metaform\scenarios\verify.demo   # one scenario, no project directory
target\debug\metaform.exe check all                               # the repository's own rules
```

## What is here

| Directory | Contents |
|---|---|
| `crates/` | Nine workspace members, all under `crates/`: the measure (`kernel`), the protocol (`propose`), the store, the runtime, the interface, and one crate per proposer (`vsa`, `sme`, `ilp`, `components`) |
| `paper/en/`, `paper/zh/` | Bilingual body text. en uses `pdflatex` twice; zh uses `xelatex` twice (ctex) |
| `proofs/` | Lean 4 formalization: 13 declarations carrying Theorems 1–11, no admitted goal and no warnings; `proofs/mechanization.md` maps theorem to declaration |
| `docs/` | The manual: commands, protocol, components, what is checked, results |
| `experiments/` | Python experiments: synthetic domains, Lean domain, code domain, cost breakdown, data ledger `RESULTS.md` |
| `scripts/` | Assertion gates, alignment and reference checks, Lean/Rust wrappers, bundle builder |

What a clone carries is `crates/`, `paper/`, `proofs/` and `docs/`, plus four documents and the build
files. `experiments/` and `scripts/` are working directories: they run beside the code and are cited by
path, but they are not versioned, so a fresh clone has the code and the claims and must rebuild the
gates. The allow list at the top of `.gitignore` is the authoritative answer to what is tracked.

## Documentation

| Page | What it answers |
|---|---|
| `docs/cli.md` | every command, the scenario-script language, and what each read-only report means |
| `docs/protocol.md` | the `metaform.propose/1` request and response, held-out predictions, the falsifier, the record `apply` leaves |
| `docs/components.md` | which crate proposes what, how the controller adopts and records, how the layering is enforced |
| `docs/verification.md` | the two gates, the claims register, the reference rules, how to reproduce every measurement |
| `docs/results.md` | the twenty checkable conclusions, the honest boundaries, and the remaining route |

The charter, `CHARTER.md` (Chinese copy `CHARTER.zh.md`), states the problem, the
thirteen claims each with a falsifier, the six positions of the runtime, the ten laws, and the route. If
this README and the charter disagree on a design principle, the charter wins; if either disagrees with the
code or the gates, the code and the gates win.

## Gates

Two, because two things are being built. `scripts/check-all.ps1` checks the program: the tests, the
experiments, and the claims about them. `scripts/check-paper.ps1` checks the paper and the
formalization: Lean, the bilingual alignment, and whether the built PDF is newer than its source. A
compiled formalization and a typeset PDF are not part of building the program, so they are not in the
program's gate; what each gate asserts line by line is in `docs/verification.md`.

## Status

The loop runs end to end on a scenario and on a project directory, and the four pieces that make it a
loop rather than a library -- the measure, the validation, the record, and the read-only reports --
are separately tested and separately gated. What is not there yet is a proposer strong enough to
reach the failure mode the paper is about: the numbers it produces are small and specific, and they
are in `docs/results.md` rather than summarized away.

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
