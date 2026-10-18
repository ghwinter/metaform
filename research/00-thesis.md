# 00 Thesis: Query-Driven Representation Change

v0.1 · 2026-10-19 · Only the thesis, no solution

## 1. Research question

One sentence: **When the target concept S lies outside the expressive range of the current representation language, any attempt to increase the search budget within that language is bound to fail; so by what signal, by what criterion, and at what cost should a system rewrite its own language?**

Split into three sub-questions that can each be answered separately:

| | Question | Type |
|---|---|---|
| Q1 | Under what conditions can we **conclude** that "the language is insufficient," rather than merely that "it has not been found yet"? | Logic |
| Q2 | There are infinitely many ways to extend a language; by what criterion do we choose among them? | Statistics |
| Q3 | Extending the language requires external signals (membership queries); how do we locate the needed predicate with the fewest questions? | Information theory |

## 2. Why it is worth doing now

1. LLMs can already **propose** predicates, properties, and relations cheaply (at the natural-language level), but there is no mechanism for deciding whether to adopt one, or which one to adopt.
2. FCA's attribute exploration solved "question-driven vocabulary expansion" long ago, at the price that attributes are supplied by humans and every question is answered by a human.
3. Between the two, only a computable selection criterion plus a noisy non-expert oracle are missing. The gap is narrow, but it is exactly fillable now.

## 3. Three claims (all falsifiable)

**C1 Necessity.** If positive and negative examples are indistinguishable under the current vocabulary (same signature), then no concept in the current vocabulary is consistent with the samples. Increasing the search budget does not change the conclusion.
→ Theorem T2, **verified in Lean 4 + Mathlib**.

**C2 Expressive power is free inside the sample and priced outside it.** The expressive capacity of a vocabulary equals 2^(the number of signatures realized on the sample); the more signatures there are, the more sample-consistent candidate concepts exist. So "expanding the vocabulary" must be bound to a bias class that resists overfitting, otherwise the consistency test is vacuous.
→ Theorem T4 (counting; the weak form has been checked in Lean).

**C3 (downgraded to a negative result) Object-level question selection is not worth much.** Under a uniform posterior, expected information gain is **exactly on par** with "ask a random question that still carries information" (1.96 vs 1.86, 300 trials); adding an Occam prior, it leads by only 9–13% (1.63 vs 1.87).
→ Conclusion: the payoff of optimization is not at "which object to ask about" but at the **vocabulary level** — when to stop asking about objects and switch to proposing predicates, which one to propose, and how much to pay for it. See `research/exp/RESULTS.md`.

**C3' Only vocabulary-level decisions carry nontrivial cost.** An object query is nearly free (one word answers it); a predicate proposal is expensive (it must be generated, a human must judge whether it is on point, and it must be verified to be decidable). Total cost is therefore determined by "when / what to propose," not by "which objects to ask about."
→ This is a testable claim: set the unit-cost ratio c_pred / c_obj to 5, 20, 100 and see whether the optimal policy switches. If "ask exhaustively first, then switch" is optimal at every ratio, this claim fails as well.

**Wording discipline**: this work does **not** propose a new selection criterion. Both MDL and information gain are off the shelf; what is new is wiring them to a **decidable trigger signal** (the candidate intension set is empty), and turning "which predicate to ask about" into a scoring problem. Do not imply in the writing that the criterion is new.

## 4. Criterion (only one)

The system's output is not an answer but **a decidable assertion**:

> Under the current vocabulary V, is S definable; and if not, which pair of indistinguishable objects is missing.

This assertion can be checked by a program, refuted by a counterexample, and proved in Lean. The remaining outputs (explanations, analogies, "insights") do not enter the evaluation section of the paper.

## 5. What would kill this thesis

1. If "vocabulary" is defined broadly enough to include all natural-language predicates, C1 becomes vacuous. → A restricted language (Boolean combinations, first-order clauses, a DSL) must be fixed and stated explicitly in the paper.
2. If an LLM that directly proposes predicates + directly judges membership is no worse than oracle-backed exploration under blind evaluation, then C3's engineering value disappears (the theory still holds).
3. If, on decidable domains (mathematics / code), representation-level competition is no better than answer-level sampling, the whole line degenerates into prompt engineering.

## 6. Three competing routes and the discriminating experiment

| Route | Claim | Mutually exclusive prediction |
|---|---|---|
| R1 Representation change (this route) | S may lie outside the language; an external signal is required to change the language | **Must fail without an oracle, even with unbounded budget** |
| R2 Causal model | S is a causal variable; interventions can determine it uniquely | When interventions suffice, no oracle is needed |
| R3 Program induction + compression | S is a program in the DSL; a large enough corpus compresses it out | When the corpus is large enough, no questions are needed |

The experiment design is direct: oracle budget ∈ {0, 5, 20}, run once in each of the three domains. The three predictions are mutually exclusive and can be separated.

## 7. Relation to 1.md

1.md gives a list of capabilities: association, induction, analogy, intent inference, meta-learning, decision-making…

This paper cuts out a single slice: **representation change**.

Reason: the other slices currently have no decidable success criterion, so writing them out can only produce prose. The slice is small, but it can be falsified, formalized, run, and published.

## 8. Honest positioning

A combination of formalism + algorithms + small-scale empirical work.

The core mathematics is folklore in FCA / ILP ("attribute-definable = union of equivalence classes"). This work's contribution is not a new theorem but:

1. using it as a **necessity criterion** (T2), which upgrades "representation change" from a heuristic to a logical necessity;
2. providing a **questioning strategy** and a **capacity constraint** (T4 / T5) that pair with an LLM predicate generator;
3. an oracle-decidable benchmark that makes "was S found" checkable for the first time.

venue: workshop / short paper preferred. The math is Lean-verifiable, the empirical work is small, and there is no pretending that this is an ICML main-conference paper.
