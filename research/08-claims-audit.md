# 08 Claim-to-Evidence Audit

2026-10-19 · Target: `paper/en/main.tex` (Chinese version kept in sync) · Method: check every number and
strong claim sentence by sentence

## 1. Claims that carry numbers

| # | Claim in the paper | Location | Evidence | Verdict |
|---|---|---|---|---|
| 1 | No concept consistent within the initial vocabulary, 300/300 | §5 Domain A | `RESULTS.md` §2 E4 (`domain_a.py --n 300`) | ✓ |
| 2 | 1.96 (information gain) / 1.86 (non-redundant random) / 6.09 (naive random) | §5 | §2 E1 | ✓ |
| 3 | 1.63 vs 1.87 under an Occam prior | §5 | §3 E5 (k=2) | ✓ |
| 4 | P(insufficient) 2.5% → 29.5% → 98.7% (k=1,4,10) | §5 | §3.5 | ✓ |
| 5 | Cost span "about 45×"; stopping rule differs by ±6% and changes sign | §5 | §3.6 (19.56 → 883.73 = 45.2×) | ✓ **changed from 44 to 45 this round** |
| 6 | Measured cost 27.6 / 49.4 / 88.9 / 162.7 = r/q + 7.6 | §5 | §3.17 | ✓ |
| 7 | Greedy 2.10 / optimal 1.87 / random 4.30 (3×3 pairs, 10 candidates) | §4 | §3.9 | ✓ |
| 8 | Domain C: 88 vs 28 (proposal cost 20) | §5 | `domain_c.py` output | ✓ |
| 9 | Proposal coverage 4/4, 2/4, 0/4, 2/4 | §5 | §3.15 | ✓ |
| 10 | Domain B: 21 integers, one Lean run 2.5 s, five queries 12.6 s | §5 | `domain_b.py` (2.52 s) | ✓ |
| 11 | Noise: at r=5, ε≤0.2 gives false triggers ≤0.5% and missed detections go to zero; at ε=0.3 both are about 8% | §7 | §3.10 | ✓ |
| 12 | θ=3 is enough for ε≤0.1 | §4 | §3.11 (0.98) | ✓ |
| 13 | Lean: 0 sorry, no warnings, about 25 s | Appendix A | `check-lean.ps1` no output, exit 0, 25.9 s | ✓ |
| 14 | Rust: 10 tests | Appendix B | `cargo test` 10 passed | ✓ |
| 15 | Reference kernel: at most 64 objects | §7 | `Mask = u64` | ✓ |

## 2. Strong claims (no numbers, but they need scope)

| Claim | Problem | Handling |
|---|---|---|
| "No system treats 'these examples we cannot express' as a decidable trigger" | negative claim, can only cover the literature surveyed | **changed this round** to "In the systems we surveyed, none treats …"; Chinese version synced |
| "Existing systems … all search only within a given representation language" | same as above | the subject is the three families FCA/ILP/program induction, named inside the sentence, acceptable |
| "Abstract: the core results are already formalized in Lean 4 + Mathlib" | accurate (9 theorems) | keep |

## 3. Claims withdrawn on our own initiative

| Original claim | When it was withdrawn | Now |
|---|---|---|
| "Picking the question by information gain saves an order of magnitude" | after the Domain A ablation (§2) | negative result: on par with non-redundant random |
| "Adaptive θ would be better" | after §3.12 | bounded by the fixed values, deleted |
| "Per-object posteriors can fix ε=0.3" | after §3.14 | slower and worse, deleted |
| The adaptive/posterior wording | same as above | only the θ∈{3,5} rule stays in the paper |

## 4. The three changes made this round

1. Cost span 44 → **about 45** (based on 19.56 and 883.73);
2. Reproduction block `domain_more_data.py # Theorem 6` → **Theorem 5** (monodata is the 5th);
3. "No system treats …" → **"In the systems we surveyed, none treats …"** (Chinese and English in sync).

## 5. Not yet verified

- ~~Appendix A's "no warnings"~~ → **closed**: `check-lean.ps1` now counts warning/sorry lines, exits with
  code 2 if there is a warning, and writes the full output to `research/lean/last-check.log`. Measured: the
  real files give `warnings=0`; a test file with one deliberately unused variable gives
  `warnings=2, exit=2` — the gate is alive, not decoration.
- ~~Rust's "10 tests"~~ → **closed**: `check-rust.ps1` asserts `passed == 10 and ignored == 0`; measured
  exit code 1 when the expected value is written wrong.
- ~~Paper numbers have no regression~~ → **closed**: `verify_claims.py` compares the six numbers of §3.18
  (2.01/1.91/6.14/2.10/1.85/4.28) against the measured ones, tolerance 0.06; `check-all.ps1` chains the
  three gates — Lean, Rust, numbers (about 30 s, all green).
- **The gates themselves were tested too**: `selftest-gates.ps1` deliberately produces three kinds of
  failure (a file with a warning, a wrong expected value, tampered paper numbers), and the three gates exit
  with 2 / 1 / 1 respectively — they are not decoration.
- Claim 5's "changes sign" depends on the specific grid of the cost experiment; a different seed may flip
  it. §3.6 already states that it is the same batch of data.
- **Variance**: the question policies (2.01±0.06 / 1.91±0.07 / 6.14±0.23) and the coverage policies
  (2.10±0.06 / 1.85±0.06 / 4.28±0.09) now have 5 seeds (`RESULTS.md` §3.18); the rest (cost r/q, the θ
  curve, domains B/C) are still single runs, but they are either structural (r/q) or deterministic
  trajectories (domains B/C), so the risk is low.
- New finding: **gain is slightly worse than rand_inf** (by 0.10 ≈ 1.5 sd). The paper's wording should be
  "no advantage", not "better".
