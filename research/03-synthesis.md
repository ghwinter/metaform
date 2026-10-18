# 03 Literature synthesis: who fixed what

v1.0 · 2026-10-19 03:30 · **coverage 36/36 papers** (six batches, all based on local text in `.library/_text/`)

## 0 Method

- 36 PDFs → `pdftotext` → `.library/_text/` (`scripts/extract-pdf-text.ps1`, 17.6 s).
- Extracted in six batches, with six fixed fields per paper: **research question / fixed representation or language / whether the representation can change / trigger signal / oracle / selection criterion**.
- Trust only the local text; write "not stated" when the text does not say it; filling in from memory is forbidden. Quotes at most ≤ 25 words.
- Known gaps: the body text of `gentner1983` is OCR-damaged and only the last page is readable; FCA's attribute exploration appears only in
  the `03a` documents, with no corresponding implementation (`concepts` package does not include that algorithm), so it needs a citation from Ganter & Wille's original work.

## 1 Comparison table (36 papers)

### 1.1 Problem finding and cognitive modeling (02)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| adolfi2024 | Not formalized; only the convention of a "language or framework" | Allowed at the conceptual level, no operators | A plausibility constraint is violated | None | Satisfies the plausibility constraints |
| kemp2008 | Graph-grammar hypothesis space (8 forms / 20 candidates) | Partial: F can change, within an enumerated set | Dataset D | D itself | max P(S,F∣D) |
| kemp2008 SI | Metagrammar templates | No (the implementation only enumerates 20) | Same as the main text | Not stated | Same as the main text |
| kemp2010 (causal) | Schema space; domain problems and predicates given | Partial: the number of categories can change | Multi-domain event data | None | Schema posterior |
| kemp2010 (theory) | Relational system (z,g); relations given | Partial: category partition and count can change | Relation matrix R | None | P(z,g∣R), CRP prior |
| tracing_insight | Initial representation of matchstick problems | Yes (by participants); the model does not produce it | Hints, eye movements | Hints given by the experimenter | None; change-point detection |

### 1.2 Structure mapping and vector symbolic architectures (03b / 03c)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| falkenhainer1989 | Typed predicate calculus, predicates must share names | No, only the rules can be swapped | None | Modeler | Structural score |
| gentner1983 | Predicate argument structure (syntax only) | No | None | Human representation | Systematicity |
| sme4_complexity | Same as above; statements are only added, never changed | No; Remap relies on the outside | External judgment | External task model | Score threshold |
| sme4_using | Requires a preloaded vocabulary | No | Not stated | User | Not stated |
| kleyko2022 (survey I) | D-dimensional HV + binding/superposition | Can add new symbols; semantics unchanged | None | Designer | Similarity, capacity |
| kleyko2023 (survey II) | Same as above; analogy requires a predicate graph | No; encoding is the bottleneck | Not stated | External preprocessing | Accuracy, cost |
| schlegel2021 | The chosen VSA space and operators | No; fixed at design time | Benchmark experiments | Designer | Capacity, accuracy |

### 1.3 Rule and program induction (04)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| cropper2021 | Clauses + hand-specified language bias (metarules / mode) | Partial: predicates can be invented, the bias is unchanged | Need for chained decomposition, clauses too long | Positive/negative examples + BK (human-given) | Coverage + shorter program |
| ellis2020 | Domain λ-calculus primitive set | Yes: library growth | Subtrees reused in the wake phase | Task I/O | Posterior + description length |
| lake2015 | Probabilistic programs (discrete action primitives) | Basically no: concepts can compose, priors can be learned | Background character set (offline) | A few sample images | Posterior |
| makke2024 | Predefined function library L | No | None (the library is chosen manually) | Data (x,y) | Accuracy / simplicity / interpretability |
| palmarini2024 | λ-calculus + library D | Yes: chunking | Recognizer usage rate + compression gain | Task + neural recognition model q | Compression objective |
| pysr | User-supplied operators, variables, depth limit | No (custom operators are written by humans) | None | Data + user configuration | Accuracy-complexity Pareto |

### 1.4 Probability, information, and model selection (05)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| di_fiore2024 | Variables χ, GP kernel, fidelity levels | No (fidelity levels ≠ a new language) | Acquisition-function peak | High-fidelity simulation / experiment | EI / PI / MES |
| farquhar2024 | Token distribution, NLI equivalence classes | No (only clusters existing meanings) | High semantic entropy | NLI judges bidirectional entailment | Semantic entropy threshold |
| friston2026 | Factorized model, preset model space M | Partial: only reduction within M | High structural EIG | Environment feedback | Expected free energy, Bayes factor |
| grunwald2019 | Model family {M_γ}, likelihood | No (selection only within Γ) | Data (passive) | Data itself | Shortest code length |
| lieder2019 | Hypothesis space, cost function | In principle yes, but no online mechanism | Accuracy-cost gap | Environmental statistics + utility | Expected utility under constraints |
| perfors2011 | Hypothesis space H, generative process | Limited yes; the latent space is unchanged | Observed data | Data | Posterior + Occam |

### 1.5 Intent, pragmatics, and causal representation (06 / 07)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| frank_goodman2012 | Shared vocabulary V, Boolean word meanings, referent set C | No | None | Measured human salience priors | Utility = informativeness − cost |
| ying2024 | Fixed goal set G, grid actions, joint planning | No | Instruction and action | Human participants, GPT-3 likelihood | Posterior P(g∣u,a) |
| scholkopf2021 | Causal variable set, graph, intervention set (treated as given) | Yes, but external interventions/shifts are required | Interventions, distribution shifts, SMS | Multi-environment intervention data | ICM/SMS sparse changes |

### 1.6 Counterexamples, self-correction, and scientific discovery (08 / 09)

| File | Fixed representation / language | Can the representation change? | Trigger signal | oracle | Selection criterion |
|---|---|---|---|---|---|
| clarke2000_cegar | Set of atomic formulas (predicates); h can change | Partial: only refines the partition of h's equivalence classes | Shortest prefix of a spurious counterexample | Concrete model K, SplitPATH symbolic verification | Coarsest separable refinement (practically suboptimal) |
| agarwal2026 | CoT tokens of the same model | No: only sampling / voting / beam search change | Compute + difficulty + model type | Evaluation ground truth | Accuracy per unit compute |
| huang2024 | Same LLM + prompts | No | Self-generated feedback | None (ground truth only for comparison) | Answer accuracy |
| madaan2023 | Same LLM + three kinds of prompts | No | Self-feedback text | None (human / GPT-4 preference) | Preference rate / task metrics |
| snell2025 | Proposal distribution + fixed verifier | No | Verifier score, difficulty binning | PRM/ORM | Verifier-weighted best-of-N |
| tacl2024 survey | LLM + feedback prompts | No | Feedback generation | External tools / knowledge / humans / ground truth | By RQ and checklist |
| abduction2022 | Conditional framework + background knowledge | **Yes**: creative abduction can introduce new terms | Fact F has no known conditional that entails it | Pattern generation / justification rules | Ordering within patterns |
| kramer2026 | Equations / ODEs + grammatical operators | Partial: library learning adds components | Fit error / complexity | Experimental data, domain experts | Accuracy + MDL + interpretability |

## 2 Original-text anchors I verified myself

1. **ILP itself admits the bottleneck is the language.** "Even for ILP experts, determining a suitable language bias is
   often a frustrating and time-consuming process. We think the need for an almost perfect
   language bias is severely holding back ILP" (`cropper2021_ilp_at_30.txt`).
2. **Kemp says predicate invention is needed, yet sidesteps it with "two senses of hypothesis space."**
   "Predicate invention is crucial since part of our goal is to discover novel concepts, and new
   predicates will be needed to refer to these concepts." (`kemp2010_probabilistic_theory_formation.txt`)
   He then defends this by saying that "search often moves to regions far from the starting point" — **a trajectory argument, not a vocabulary argument**.
3. **Vocabulary growth exists, but it is corpus-driven.** "as the model gains a richer conceptual vocabulary, its
   representations evolve" (`ellis2020_dreamcoder.txt`); the criterion is compression.
4. **Active inference itself admits the model space is unsolved.** "An outstanding issue is the elaboration of alternative
   models; i.e., the specification of the model space." (`friston2026_active_inference.txt`)
5. **Bayesian learning only selects among existing hypotheses.** "all the model does is choose among hypotheses that already
   exist" (`perfors2011_bayesian_tutorial.txt`)
6. **CEGAR refines the partition, not the vocabulary.** Atomic formulas are "automatically extracted from the program";
   and "The problem of finding the coarsest refinement is NP-hard" (`clarke2000_cegar.txt`).
7. **The only normative mechanism for "coining new words" is creative abduction.** "introduction of a new term that is hitherto
   undefined" (`abduction_conditionals_2022.txt`) — but there is no criterion for deciding whether to coin one, or which.
8. **On the LLM side there is no ability to change the language at all.** Six papers (four on self-correction + two on test-time compute) agree: feedback is self-scored from the same source,
   and the language does not change; the survey's own words are "no prior work demonstrates successful self-correction with feedback
   from prompted LLMs" (`tacl2024_self_correction_survey.txt`).

## 3 Gap assessment (final version after full coverage of 36 papers)

| Dimension | Status quo | Gap |
|---|---|---|
| Trigger signal | Compression stagnation, reuse frequency, coverage gain, posterior, spurious counterexample, fit error, plausibility constraint | None is the **decidable** signal "the candidate intension set is empty" |
| Language revision operator | predicate invention (no criterion), library refactoring (corpus-driven), h partition refinement, creative abduction (no criterion) | Missing the operator "triggered by a single query, selecting the next predicate by information gain" |
| Selection criterion | MDL / posterior / coverage / structural EIG / expected utility / Pareto | None is used to choose "which predicate to ask about next" |
| oracle | Human experts, task sets, model checkers, NLI, image labels | Missing the combination "cheap noisy oracle + active question selection" |

**Final conclusion**: among these 36 papers, **not one treats "the current language cannot express it" as a decidable trigger and pairs it with a questioning strategy of reasonable symbolic cost**. The three closest:

1. FCA attribute exploration (`03a` has only the library, no algorithm implementation; the original work must be cited) — humans give attributes, humans answer questions;
2. ILP predicate invention — it can coin words, but has no criterion;
3. Creative abduction — it can introduce new terms, but has no trigger criterion.

Together these three are exactly the algorithmic skeleton of this paper, and they have never been synthesized.

## 4 Impact on the paper (three items, all landed)

1. **§6 related work** has been rewritten: Cropper's exact words on language bias, Kemp's predicate invention and "two kinds of hypothesis space,"
   DreamCoder's corpus-driven refactoring, Friston's outstanding issue, Perfors's "choose only among existing hypotheses."
2. **T2 formulation** changed to trajectory vs vocabulary, answering Kemp's trajectory defense directly.
3. **Contribution 2 wording** changed to "wiring an existing criterion to a new decidable trigger signal," not passing itself off as a new criterion.

## 5 Experiments: domain A (synthetic formal contexts)

Full data and interpretation in `research/exp/RESULTS.md`. Three-sentence version:

1. **The empirical version of T2 holds**: when the initial vocabulary lacks one distinguishing attribute, hypotheses consistent within V₀ are **empty 300/300** times.
2. **C3 is void**: under a uniform posterior, "information-gain question selection" and "ask a random question that still carries information" are exactly on par
   (1.96 vs 1.86; product space 7.78 vs 7.78). The entire gain comes from "not asking uninformative questions," not from "choosing cleverly."
3. **Only a prior gives a slight edge**: with p(S) ∝ 2^-|S|, 1.63 vs 1.87 (9–13%). Bayesian active learning is a weak lever in this domain.

Direct impact on the paper: the main metric changes from "number of questions" to **recovery rate under total cost (object queries + predicate proposals)**,
and we compare three stopping rules (switch only when F is empty / switch only when the budget is exhausted / switch by expected payoff). This is consistent with T2:
since search within the vocabulary must fail, optimizing object-level question selection is optimizing a process doomed to fail.

## 6 Next steps

1. Fix the wording of C3 (the item above).
2. Expand domain A to 8 attributes × 14 objects and add an "LLM intuition question selection" baseline, to see whether 1.62× scales up.
3. Hook up domain B (Lean judgment) / domain C (test judgment).
4. Add the FCA original citation (attribute exploration), otherwise the first paragraph of §6 has no source.
