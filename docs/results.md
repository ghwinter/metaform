# Results, boundaries, and the remaining route

Every number below is reproduced by a command in `docs/verification.md` or by `check-all`. The
sentences that state a number are also the ones `experiments/verify_claims.py` parses, so the paper,
this page and the program cannot drift apart silently.

## Conclusions (checkable)

1. **T2 (necessity)**: if a positive and a negative example share the same signature class, then no consistent concept exists in the current vocabulary. Reproduced 300/300 in the synthetic domain.
2. **T5 (irreparable)**: more examples do not fix an insufficient vocabulary, they only expose it. P(insufficient) goes from 2.5% to 98.7% as the number of examples grows.
3. **Question selection is a second-order term**: information gain ties with "ask a random question that still carries information"; even with a prior it leads by only 9–13%.
4. **Cost is determined by proposal quality**: total cost spans 45× and is almost entirely determined by `r/q`; stopping rules differ by ±6%.
   `verify_claims.py` checks three things: the relative error of `r/q+7.6` against the four measured points is <4%; the word
   `exactly` must no longer appear in the paper; and it actually runs `metaform ops` to check `3.11/3.50/3.60` and the probe's `1/5/14`, then parses
   the same numbers out of `docs/results.md` as an independent third-party comparison (paper, this page, and the program must all agree).
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
15. **One end-to-end run works**: `crates/metaform/scenarios/verify.demo` (6 test files, deciding "is this an integration test") goes through
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
20. **A review policy fits its taxonomy except in one place**: the pre-revision manuscript was given, blind, to two reviewers with the same checklist. They agreed on 14 of 16 findings (Cohen's kappa 0.67); the author's own decisions agree with them on 11 and 9 of the 16 (kappa below zero), so those labels are not a stable policy and are not used here. On the 14 agreed findings, the taxonomy (abstract, numbers, cross-references, titles, captions, terminology, typography, notation, voice) leaves the state exhausted: the voice findings split between "proved" (rejected — a probe refutes deductively) and tense or register (adopted), giving colliding pairs (11,13) and (12,13). One predicate, claim strength, is enough to identify the rule. `crates/metaform/scenarios/review.demo` reproduces both states.

21. **The capability floor is a precondition, not a footnote**: on seventeen omission tasks the local model produced Rust that does not compile in eight to eleven of them -- type errors, not omissions -- and of the six runs that did compile, only one handled the omitted case (2/15, Wilson [3.7%, 31%]). A judge cannot repair a proposer that cannot write the function, so the share of failures this device addresses has to be measured before the device is credited with any.
22. **Naming is not handling**: asked to name the region a specification leaves undefined, the model succeeded in 13 of 17 tasks. The regions were visible to it; it had no reason to look at them. This is a salience problem, and it is cheap to fix with a schema -- a fixed slot that must be filled, independent of the topic.
23. **A deterministic proposer beats the model that is actually asked**: asked for a predicate over a 36-object world, the model answered with the two objects under discussion and was refused six times out of six; a self-implemented inductive learner, given the same vocabulary and the same evidence, scored a hit rate of 0.900 against the model's 0.200, with all ten of its candidates accepted against five of the model's, at 29.8 cost units against 107.6. Proposing need not be a language model's job; the open question is whether the criterion is doing any work, which has never been tested by switching the vote-gap gate off.

24. **Naming the unstated cell is not handling it**: with the seventeen omission specifications turned into a cell table -- the cells each specification already decides, the cell it hides, and the keywords that find each cell in prose -- the model named a cell the specification had not decided in 14 of 17 tasks, and named the hidden cell itself in 12 of 17, while implementing it in 1 run out of 15 that compiled (complement coverage 0.611 at cell level, prophecy hit rate 0.333, handling rate 0.133). The distance between saying and doing is the whole remaining distance, and the asked arm is worse at it than the silent arm (1 of 6 against 1 of 9).
25. **A gate can catch a corrupted label, and the reason is the re-ask**: re-asking the objects a candidate depends on, and refusing the candidate when the votes flip, turned nine false admissions into none. But a control arm with the same gate and no retrieval -- re-asking every published negative -- caught the same 9 of 9 at 27 questions against the probe's 26. The retrieval was not what made the gate work; it chose which class to re-ask, and barely narrowed it. The cost is 26 questions against 9 proposals, and the cost law has no place yet for questions spent checking the evidence.

## What the loop has actually run against

The proposer is a 3.8B local model, Phi-4-mini Q4_K_M, behind `experiments/local_proposer.py`. Pooled
over runs, the witness pair moves recovery 15–20 points over placebo (58% against 39%, p = 0.007); a
single nineteen-instance batch is worth anywhere from 32% to 68%; and a six-step chain produced a +3
point gap against a pre-registered bar of 15.

**As of 2026-10-24 the model has also been run where it matters most, and two limits showed up.**
It names the unstated region in 13 of 17 omission tasks but handles it in 1 of the 6 that compile, and it
cannot write a predicate over a 36-object world -- it answers with the pair under discussion. A
deterministic inductive proposer, given the same evidence, does produce the global extension the model
cannot. So the open item is no longer the adapter: it is whether the criterion, rather than the
proposer, is what makes the difference.

## Honest boundaries

- The core mathematics is folklore from FCA/ILP ("definable = union of equivalence classes"). The contribution is to use it as a **necessity criterion**
  and to supply the typing of multiple operators (paper §4: monotone, idempotent, the only one that decreases, recombination does not break collisions), with triggers, baselines, and negative results following.
- All experiments are on synthetic domains and one small code domain, with **no real LLM proposer** in the loop for the results above; the paper scopes its claims accordingly and does not overclaim.
- The counting form `2^m` of T4 is formalized as `capacity_bound_strong`; the weak form is kept as a lemma.

## Remaining route

The release stops at a validated boundary, not at a runtime. What is left, in order:

1. **Lean that a clone can recompile.** The mechanization check borrows a Lake project whose path is
   machine-local, so a clone can read the proofs but cannot recompile them. Track the lakefile and the
   toolchain (Mathlib revision included), generate the manifest, and let `check-paper` run `lake build`
   in place -- which is also what makes Lean incremental: Lake rebuilds a module only when its own
   source or a transitive import changed, and that is a property of its build graph, not of `lake env
   lean` on a single file. Acceptance: a clone with only `lake update` reports the same 13 declarations,
   no warnings, and a second `check-paper` run does not re-elaborate the proof.
2. **A proposer strong enough to reach the failure mode.** The open item is a proposer that can
   substitute a nearby problem for the one it was asked, which is the only way the count and the
   witness come apart. Acceptance: the same four arms at equal budget on a model that clears that floor.
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
