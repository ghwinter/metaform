# 02 Paper skeleton: Vocabulary Before Search

v0.1 · 2026-10-19 · body in English, notes in Chinese

## 1. Title (three candidates, pick one)

1. **Vocabulary Before Search: Query-Driven Representation Change for Concept Discovery**
2. *When More Search Cannot Help: A Necessity Criterion for Vocabulary Growth*
3. *Learning What to Ask: Attribute Exploration with Machine-Proposed Predicates*

Use 1.

## 2. Abstract skeleton (five sentences)

1. Recovering an unstated concept S from a few examples is a core task of induction; existing systems (FCA, ILP, program induction) all search **within a given language**.
2. We prove that when positive and negative examples are indistinguishable under the given vocabulary, no consistent hypothesis exists in that language, and **increasing the search budget does not change the conclusion** (T2).
3. The question therefore shifts from "how to search better" to "how, by what signal, and at what cost to rewrite the language."
4. We give a three-piece kit: a definability criterion (T1), a capacity upper bound (T4), a query lower bound (T5), and an exploration algorithm in which an LLM proposes predicates, a program selects queries by information gain, and an oracle adjudicates.
5. On three oracle-decidable domains we measure the number of questions needed to identify S and held-out accuracy, and compare against fixed-vocabulary full-budget search. The core theorem has been formalized in Lean 4 + Mathlib.

## 3. Contributions (three, no more)

1. **Criterion**: a decidable "language is insufficient" signal (the candidate intension set is empty), which upgrades representation change from a heuristic to a logical necessity.
2. **Algorithm + constraints**: an exploration loop that separates predicate proposal, query selection, and the capacity constraint; and why consistency alone is not enough (T4).
3. **Benchmark**: three oracle-decidable domains that make "was S found" checkable for the first time; core theorems formalized in Lean.

## 4. Sections and volume (target 8–10 pages + appendix)

| Section | Content | Pages |
|---|---|---|
| 1 Intro | Phenomenon: given A/B/C/D, the system only circles in nearby semantics; why search within a fixed language must fail | 1 |
| 2 Setting | Vocabulary, signature, definability, query, candidate intension set (§1 of 01) | 1 |
| 3 Results | T1–T5, proofs in the appendix; the main text keeps only T2 and the corollary of T4 | 2 |
| 4 Algorithm | Exploration loop, `F = ∅` trigger, information-gain query selection, restricted language for predicate proposals | 1.5 |
| 5 Experiments | Three domains × baselines × oracle budget | 2.5 |
| 6 Related work | Who fixed what (see the table in §6) | 1 |
| 7 Limits | Noisy oracle, undecidable natural-language predicates, single-step growth | 0.5 |
| Appendix | All proofs + Lean file list + reproduction scripts | — |

## 5. Figures and tables

- **Fig 1** The representation–query loop (block diagram): `V → F → (F=∅? expand : ask) → oracle → F`.
- **Fig 2** A concept lattice of a formal context, visualizing "when the vocabulary is insufficient, positive and negative examples fall in the same cell." **This figure is the persuasive core of the paper**: a reviewer understands T2 just by looking at it.
- **Fig 3** Questions vs recovery-rate curves (information gain vs random vs LLM intuition).
- **Table 1** Related-work comparison (§6).
- **Table 2** Main experimental results table.

## 6. Related-work comparison table (use the materials in the library directly)

| System / theory | What is fixed | Allowed to change the language? | oracle | Selection criterion | Location in library |
|---|---|---|---|---|---|
| FCA attribute exploration | Attribute vocabulary (except additions during exploration) | Yes, but humans supply attributes and answer each question | Human expert | Implication-failure driven | `03a_fca` (concepts doc + zip) |
| ILP | Predicate signature | Needs predicate invention; no criterion | Labeled data | Coverage + compression | `04_rule_program_induction/cropper2021_ilp_at_30.pdf` |
| SME | Predicate vocabulary | No | — | Structural consistency + systematic mapping | `03b_structure_mapping` (+ SME4 source) |
| DreamCoder | DSL (which it refactors) | Yes, driven by **corpus** compression | Task set | MDL / compression | `04_rule_program_induction/ellis2020_dreamcoder.pdf` |
| Kemp structural form | Form family (tree / chain / ring / order) | Selects among forms, driven by the **dataset** | — | Bayesian model selection | `02_problem_finding/kemp2008_discovery_structural_form.pdf` |
| VSA / HDC | Binding / bundling operators | No | — | — | `03c_vsa_hdc/kleyko2022_vsa_survey_part1.pdf` |
| MDL / Bayesian model selection | Hypothesis space | No | — | Description length / posterior | `05_probability_model_selection` |
| CEGAR | Abstraction layer | Yes (abstraction refinement) | Model checker | Counterexample driven | `08_counterexample_llm` |
| RSA / intent inference | Plan space + utility | No | — | Bayesian inverse planning | `06_intent_pragmatics` |
| LLM sampling | — | No | — | None | Baseline |

**The gap in one sentence**: no system treats "because these examples cannot be explained, replace the language that describes them" as an operation that is **triggered by a single query, decided by a program's criterion, and powered by a cheap oracle**.

## 7. Experiment matrix

Three domains × four baselines × three oracle budgets (0 / 5 / 20 questions).

**Domain A (main experiment, synthetic)**: formal contexts, 20–50 objects, hidden S defined by some attribute combination; the initial vocabulary `V₀` deliberately lacks one distinguishing attribute. S is fully known, so recovery is exactly decidable.

**Domain B (mathematics)**: given several integer sequences / structure examples, guess an algebraic property; oracle = Lean checks the proposition.
Note: Lean cold start `import Mathlib` takes 448 seconds, so a **batch script** is mandatory and there is no interactivity.

**Domain C (code)**: given several input–output examples, guess a function invariant; oracle = running tests.

**Baselines**: ① single-shot LLM ② multi-candidate sampling ③ fixed `V₀` full-budget search (the empirical version of T2: even with the budget maxed out, recovery fails) ④ this method · three stopping rules (switch only when the candidate set is empty / switch only when the budget is exhausted / switch by expected payoff).

**Metrics**: **recovery rate under total cost** (cost = object queries × c_obj + predicate proposals × c_pred, with ratios 5 / 20 / 100), exact recovery rate, number of representation changes, held-out accuracy.
Reporting "number of questions" alone is no longer enough — domain A has already shown that object-level question selection is nearly free (see `research/exp/RESULTS.md`).

## 8. Five things a reviewer will ask in rehearsal

1. **"The math is folklore."** Acknowledged. The contribution is the criterion, the algorithm, and the benchmark, not new theorems. Lean formalization is there to make the "criterion" machine-checkable, not to show off.
2. **"Hasn't FCA attribute exploration already done this?"** Three differences: attributes are proposed by an LLM rather than given by humans; the oracle is cheap and noisy; queries are chosen by information gain rather than by lattice order.
3. **"Kemp 2008 already selects among forms."** That is corpus-driven, not driven by a single query; and the form family is a finite set enumerated in advance.
4. **"The experiments are too small."** Acknowledged; declared as a feasibility study, with a scalable protocol (domain A can auto-generate tens of thousands of instances).
5. **"LLM-proposed predicates are not decidable."** Restrict the domain: handle only predicates whose membership relation is decidable; natural-language predicate experiments are used only for blind evaluation and do not enter the main conclusions.

## 9. Order and timeline (personal-project scale)

| Week | Tasks |
|---|---|
| 1 | Write the theorems of 01 into a note; bring T1–T3 into Lean; draw Fig 2 |
| 2 | Bring T4/T5 into Lean; first draft of paper §1–§3 |
| 3 | Generator for domain A + four baselines; run the first batch of numbers |
| 4 | Hook up domains B/C; first draft of §4–§5 |
| 5 | Related work + limitations; full read-through; LaTeX typesetting (`pdflatex` twice, **do not use latexmk**) |
| 6 | Internal review, additional experiments, submission version |

## 10. Relation to the code (theory first, code later)

Code appears in only two places, and both are late:

1. **Experiment scaffolding** (Python is faster: numpy/pandas are already there, and FCA has ready-made libraries) — written only in week 3.
2. **The engine proper** (Rust is more suitable: state machines, search, determinism, performance) — written only after the paper has results.

Reason: writing code now amounts to reifying objects that are not yet clearly defined, and the rework cost exceeds the benefit.
