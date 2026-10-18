# 07 JEV / System One: Where It Sits in Our System

Materials: 8 items on System One and decision models.

## 1. What it is

TypeSafe AI's System One model, with three kinds of primitives: **Noul** (a binary judgment that returns
the probability of being true), **Choice** (pick from a given set of options, returning a distribution
over options), **Score** (rate on a graded scale). A single call can ask multiple independent questions
about the same context in parallel; the weights and architecture are undisclosed, and it is usable only
through the API.

Key limitation: **the option set is given**. It can choose; it cannot invent.

## 2. Five empirical findings (each with the source quote)

| Finding | Source |
|---|---|
| Once the threshold is frozen, "accept the confident verdicts and escalate the rest" is 0.9 points more accurate than GPT-6, at 41% of the cost | the 2026 study of JEV as a judge: "accepting confident verdicts and escalating the rest is 0.9 points more accurate than GPT-6 on 1,610 held-out pairs at 41% of its fee" |
| A decision model's standalone accuracy ≠ agent success rate: **handing tool choice to it without fitting the threshold drops the success rate by 19.0 points** | the 2026 study of JEV in an agent harness: "without a fitted confidence threshold, delegating tool choices lowered success by 19.0 points" |
| Choice is sensitive to equivalent phrasings (0.771–0.978); calibration must be validated per task | the 2026 System One calibration study (v1) |
| No consistent advantage over LLMs (no advantage over Qwen) | the 2026 study comparing JEV with LLMs on political-science tasks |
| The advantage depends on task, option count, threshold, and calibration method | the 2026 study of System One decision gates |

## 3. Where it sits in our system

The three-layer division lines up exactly:

| Layer | Who does it | JEV's place |
|---|---|---|
| Candidate generation | LLM / structural mapping / metarule | **Cannot** use it — the option set is fixed |
| Meta-reasoning decisions | Choose the next operator, judge whether verification is worth it | **Can** use it, and it can supply a confidence |
| Verification feedback | Tests / Lean / execution | Cannot use it (same-source self-assessment) |

This is not the same thing as the three-way separation in my paper (proposal / selection / adjudication):
what JEV can replace is **selection**; it cannot replace **proposal**, and still less **adjudication**.

## 4. Two things that can be changed directly

1. **The oracle's return type.** In my Rust kernel, `Oracle::ask` is currently `Option<bool>`, which
   distinguishes only "knows / does not know". JEV's usage calls for three states: **true / false /
   insufficient confidence → escalate**. That maps exactly onto the "cannot decide" slot in T2: not a
   guess, but a mark of "pending verification".
2. **Add an `escalate` field to `need`'s JSON**: when the verdict confidence for certain collision pairs
   falls below the threshold, the engine should output "these pairs need external adjudication" rather
   than keep asking the object.

## 5. One warning (from the harness paper)

"More accurate at every step" does not equal "better overall". This is isomorphic to the negative result
in my Domain A: **improvements in a local metric get eaten by a wrong global assumption**. So any JEV
integration must be tested end to end, and what it measures is "the number of proposals / rounds needed
to recover the target concept", not "per-step selection accuracy".

## 6. Conclusion

JEV is a replaceable **decision backend**, not the core of this system. The core is still: structural
representation, candidate-rule search, model selection, and counterexample-driven self-correction. The
integration is one replaceable `Decide` interface, not something written into the theory layer.
