# Mechanization map

Which Lean declaration carries which statement of the paper. The paper states the results and
cites the prover; this table is the maintenance record that keeps the two in step.

The development is `proofs/QueryDriven.lean`. It compiles with no admitted goal and no
warnings; `scripts/check-lean.ps1` fails on any warning.

| Result | Lean declaration | Note |
|---|---|---|
| Theorem 1 | `definable_iff` | characterization by classes |
| Theorem 2 | `no_consistent_concept` | necessity (main result) |
| Theorem 3 | `indist_mono`, `indist_insert` | monotonicity of growth |
| Theorem 4 | `capacity_bound_strong` | counting form ($2^m$) |
| Theorem 5 | `no_consistent_concept_mono` | more data does not repair |
| Theorem 6 | `query_lower_bound` | counting argument, $|\mathcal{F}| \le 2^k$ |
| Theorem 7 | `no_collision_of_separated` | covering criterion (set cover) |
| Theorem 8 | `phi_mono` | generation cannot reduce the live set |
| Theorem 9 | `phi_closure_idem` | closure is idempotent |
| Theorem 10 | `phi_evidence_lt` | evidence strictly reduces the live set |
| Theorem 11 | `recomb_cannot_separate`, `indist_insert_of_eq` | recombination keeps collisions |

Not listed, because they are supporting lemmas rather than paper statements: `sig_apply_of_mem` and
`sig_apply_of_not_mem` (used by the monotonicity proof) and `capacity_bound` (the weak counting form,
kept as a helper for `capacity_bound_strong`). The definitions `live` and `Phi` are the state the
operator statements are phrased over.

`scripts/align-papers.py` checks this table: every declaration named here must exist in the Lean
file, the number of entries must equal the count stated in both papers, and the table must stay in
step with the theorem list.
