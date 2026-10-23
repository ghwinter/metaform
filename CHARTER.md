# 00 Charter: Metaform's Design, Idea, and Route

v1.0 · 2026-10-20 · Distilled from three research conversations. This is a specification, not a summary.

The charter compresses three conversations into one executable stance: what problem this is, what we
claim, what order the system has, what the code may not do, where the route goes, and how it dies. It does
not explain background; it states conclusions and their consequences. Background begins at 01.

Read §1 and §2 for the problem and the claims; §4 and §6.3 are the parts in force today. The rest is
reference.

## 0. Marks and authority

| Mark | Meaning | How it is judged |
|---|---|---|
| **[theorem]** | proved, compiled with the repository | Lean 4 + Mathlib, 13 declarations carrying Theorems 1–11 |
| **[evidence]** | this repository has run it and has numbers | scripts under `experiments/` reproduce them |
| **[wager]** | belief without evidence | can only be cashed in or abandoned by later experiments |

Authority: **code and the gates > this file > any note**. The charter obeys its own L9: every
checkable sentence it writes either has a check in the repository or is explicitly marked **[wager]**.

One writing discipline: every principle states **what it forbids**. A principle without a prohibition is
a wish.

## 1. The problem

### 1.1 One observation

Give a model thirteen parallel preferences a–m — faster, less memory, fewer copies, more modular, more
elegant, more kernel-like, and so on — and the answer stays inside the semantic neighbourhood of a–m.
Point that out and the answer discusses *why* it stays there — still without stepping outside a–m.

This is not a knowledge deficit. The system knows what "code aesthetics" activates. It does not know that
a–m are samples of a set that also contains n, o, p, and one rule binding them together.

Split understanding into three levels and the failure has a location: **① concept association** (generate
around the activated words) → **② structural inference** (derive the siblings) → **③ structure-based
decision** (use the structure to choose the next step). The first answer stops at ①; the project needs ②
and ③.

### 1.2 Five projections

In three conversations the same question was put five ways, and each time it was one thing:

| Given (the sample) | Wanted (the generator) | The failure complained about |
|---|---|---|
| thirteen preferences | the principle of "approaching an ideal system while the constraints keep changing" | answers circle the nearby semantics |
| last turn's answer A | the answer S that actually finds the problem | A stops near the concepts the words activate |
| documents, prompts | computable states and operators | word games, not computation |
| four elements A, B, C, D | the intension of the set S | association is not induction of a rule |
| knowledge, model scale | the ability to notice that a structure exists | knowledge is not induction or decision |

Meta-observation: **the five turns re-enact the phenomenon they describe.** Five times the same object was
given from a different side; five times the answer stayed inside that side, and not once did it say "this
is the same question".

That meta-observation is the project's starting point. If even "this is the same question" has to be
pointed out, then pointing it out should be a computable act, not a better phrasing.

### 1.3 A and S

A is the first answer. It is not wrong; it almost never points at S. S is the answer that "finds the
problem I wanted to know about".

The decisive difference: **S is defined by the asker, and the asker cannot say what it is.** The system has
no internal criterion for a hit; the only oracle is the "yes, that's it" on seeing it.

The engineering consequence is hard: if the only signal is one cheap external judgement, then the
system's main output should be **discriminating candidates and questions that cheaply elicit that
signal** — not a longer answer. This rewrites meta-reasoning from a generation problem into an
experimental-design problem.

### 1.4 The missing layer

The conversation stopped at two layers: content (what is said) and mechanism (why it is said, when it
holds). The third layer was missing: **representation** — which vocabulary states the mechanism.

The same four elements have infinitely many intensions, and the data does not choose among them. So the
engine's first primitive is not search but **re-representation**: change the attribute vocabulary, invent a
predicate, restructure the DSL, change the role–filler decomposition. Search is only effective inside a
given representation.

This yields the acceptance test that runs through the whole document: **a statement enters the project
only once it can be written as a program and run for numbers; until then it is still a document.** It also
explains why a more elegant phrasing is not enough — elegance is not a criterion, computability is.

### 1.5 Why this is not only an AI problem

"Not understanding" is isomorphic between two people and between a person and a model: the listener
receives a–m and not the mechanism running through them. Why some people are quick and others are not
comes down, in the asker's intuition, to one thing — whether they notice that some structure exists and
can be recognised and induced. **Thinking of it versus not thinking of it is the visible difference.**

That is the project's wager, not its theorem:

> **[wager]** A large part of the difference in intelligence is a difference in representational
> reachability, not in compute.

If the wager is wrong (at equal representation, compute and data still dominate), the project degrades
into a better debugging tool. If it is right, an external engine has a first-principles place.

### 1.6 One sentence

> When the target concept lies outside the current representational language, any effort to increase the
> budget inside that language is bound to fail; what the system needs is a decidable "the language is
> insufficient" signal, a criterion for choosing a new language, and a cheap external oracle.

A shorter version is worth keeping separately:

> **When the language is insufficient, stronger search gives you a more confident wrong answer.** An
> inducer that only ever returns its "least bad" clause will, under a thin vocabulary, hand back a
> structurally complete rule stated with full confidence.

## 2. Thirteen claims

Each claim has four parts: statement, grounds, design consequence, falsifier. A claim whose falsifier
cannot be written should not exist.

### 2.1 About the problem

**P1 When representation is insufficient, budget is useless.** [theorem]
If the positive and negative examples share a signature under the current vocabulary, no concept in that
vocabulary is consistent with the samples; more search does not change the conclusion.
Grounds: Theorem 1 (characterisation) and Theorem 2 (necessity), compiled in Lean. Consequence: the
engine must be able to answer `INSUFFICIENT` and stop searching, rather than sample more or rephrase.
Falsifier: a sample whose target is definable in the vocabulary but is judged insufficient.

**P2 The trigger must be decidable.** [theorem + engineering]
"The language is insufficient" cannot be a feeling; it must be a predicate: the live set is empty (Φ = 0),
or a colliding pair exists (a positive and a negative with the same signature).
Consequence: the controller has exactly three outputs — `INSUFFICIENT` (propose a predicate),
`UNDETERMINED` (ask an object), `IDENTIFIED` (stop or verify). "Think again" is not a state. Falsifier: a
state where the engine can say neither insufficient nor undetermined, and can only continue.

**P3 A sample is an extension; the target is an intension.** [evidence]
The elements the user gives are samples of an extension; what is wanted is the generator. Consequence: the
data structures carry both objects and the intension/extension of the set they belong to, and never
mistake one for the other; the report separates "known, candidate, undetermined". Falsifier: the system
outputs new elements and no rule that generates them.

**P4 A → S is a hit problem.** [evidence]
The first answer almost never hits; a hit depends on an external judgement and cannot be confirmed
internally. Consequence: outputs are questions and candidates that discriminate, in that order; length is
not quality. Falsifier: candidates chosen by information gain are no likelier to be judged "that's it"
than random ones.

### 2.2 About evaluation

**P5 Only a decidable domain can measure "found it".** [evidence]
Mathematics, code (tests), and a one-word membership answer are decidable; "is this answer profound" is
not. Consequence: every experiment binds to an oracle; a demonstration without an oracle is a
demonstration, not a result. Falsifier: more compute spent on undecidable demonstrations than on decidable
ones.

**P6 Representation-level competition should beat answer-level sampling.** [wager · the core claim]
At equal budget, competing at the level of representations hits "that's it" more often than sampling more
answers.
Grounds: only indirect evidence so far — the negative result on object-level questions, and the two-arm
pilot where the *chosen label*, not the proposal machinery, produced the entire gain. Consequence: every
experiment is designed to separate these two; an experiment that cannot is not run. Falsifier: answer-level
sampling is no worse at equal budget, and the whole route collapses into prompt engineering.

### 2.3 About the machine

**P7 The computable version of "smart" is detecting that the current representation is lossy.** [wager]
Observable signals: an unexplained residual, compression stalling, a lattice boundary violated, competing
candidates disagreeing, a counterexample that no current predicate can state. What can be mechanised is
the **detection**; which representation to switch to still requires a biased search. Consequence: the
detector is a first-class citizen, not a debugging feature. Falsifier: on known cases that require a new
representation, these signals do not appear, or the false-positive rate makes them useless.

**P8 The valuable decision is at the vocabulary level, not the object level.** [evidence]
An object question is nearly free (one word answers it) and pays about as much as a random question that
still carries information; a predicate proposal is expensive (it must be generated, judged on point, and
verified decidable).
Grounds: the negative result (1.96 against 1.86) and the cost law r/q+7.6. Consequence: the controller's
central decision is when to stop asking objects and switch to proposing predicates, which predicate, and
how much to pay. Falsifier: set the predicate/object cost ratio to 5, 20, 100; if "ask exhaustively, then
switch" is optimal at every ratio, this claim falls too.

**P9 Thinking is a transformable state plus typed transitions.** [theorem + evidence]
State s = (H, V, E, B); an operator carries a type (rd, wr, dir), a warrant, a falsifier, and a cost; the
five classes are generation (writes H, monotone), closure (filters H, idempotent), representation (writes
V, the only class that admits candidates which were inexpressible), evidence (writes E, the only class that
strictly lowers Φ), and control (writes B and selects among the rest).
Consequence: any "thinking operation" must first answer what it reads, what it writes, in which direction,
why it is admissible, when it is void, and what it costs. Falsifier: a necessary cognitive operation that
fits none of the five classes.

**P10 The novelty is in the controller.** [evidence]
The operator set is not new; what is new is which one is chosen in which state, when to stop, and why.
Grounds: closure is idempotent (thinking again is not a step), evidence is the only strict reducer, and
generation is monotone — together these say that "run a few more operators" does not converge by itself.
More operators is not more generality either: the usual six gaps are representation, discovery, control,
learning, transfer, and autonomy, and adding operators fills only part of the first two. Consequence: the
controller and the record are the product; the operator library is a part. Falsifier: a fixed pipeline plus
more model calls is no worse at equal budget than dynamic scheduling.

### 2.4 About honesty

**P11 A proposal is adopted only after validation.** [engineering]
An external candidate may not overwrite the state; it passes validation, then adoption; unvalidated
conclusions keep their hypothetical status and their source. Consequence: one write path; a report that is
not adoptable can be seen and recorded, never adopted. Falsifier: direct adoption succeeds no less often
than validated adoption, so validation only adds cost.

**P12 Knowledge is booked in three kinds.** [engineering]
Verified (a command or a proof), statistical heuristic (works but a counterexample can overturn it), and
unverified hypothesis. Mixing them into "experience" is forbidden. Consequence: the record format
separates the three; only verified entries may enter a policy. Falsifier: a policy becomes the default
without ever being verified.

**P13 A theory is a component: ask first what it fixes.** [epistemology]
Every theory fixes something and therefore cannot do something: FCA fixes the attribute vocabulary, ILP
fixes the predicate signature, structure mapping fixes the relation types, VSA fixes the encoding and
dimension, and program induction's DSL restructuring is driven by a corpus rather than by one query.
Consequence: every component enters with one line — what it fixes, therefore what it cannot do. Falsifier:
a component whose fixed item does not limit what it can express.

## 3. Order

The system needs six positions and one protocol. A position may stand empty; the protocol may not be
bypassed.

Six kinds of thing are never conflated: elements, properties and relations, structures and models,
operations and transformations, inference rules and evaluators, control policies. Treating an operator as
an element, or an evaluator as a rule, is the most common conceptual accident in systems of this kind.

### 3.1 State

Mathematical core: s = (H, V, E, B) — a finite family of candidate intensions, a vocabulary, evidence
(positive and negative), and a budget.

The engineering state adds three things: a structure graph (nodes and typed edges), goals with unresolved
gaps, and history with sources. Together, (G, H, E, Q, B); V is not one of those five slots, because what
it decides is which of the candidates in H are expressible.

Three hard constraints:

1. Every edge has a type and a semantics (causes, precedes, part of, supports, contradicts, calls); the
   arrow alone is not a semantics.
2. Every conclusion carries a source and an uncertainty; a conclusion without a source is a candidate.
3. The state is the only truth; text is input, output, and explanation.

### 3.2 Operators

| Class | Writes | Property | Typical operators | Failure condition |
|---|---|---|---|---|
| generation | H (by inclusion) | monotone, never lowers Φ | induction, abduction, analogy | the candidate is not expressible in the current vocabulary |
| closure | H (by filtering) | idempotent, lowers Φ at most once | deduction, consistency check | re-running under unchanged evidence |
| representation | V, and through it H | the only class that adds expressible candidates | propose a predicate, add a relation, change a decomposition | the new vocabulary does not preserve existing distinctions |
| evidence | E | the only strict reducer of Φ | ask, experiment, counterexample search | the oracle is undecidable or the noise exceeds the bound |
| control | B, and selects among the rest | does not change Φ directly | budget allocation, stopping | without a cost model it degenerates to random |

The type is not decoration: `wr` (what is written) takes four values and cannot separate generation from
closure; the direction is needed with it.

Two things are fixed, one is not: **the five classes are fixed** (every step falls into one), **the
concrete operators are grown** (no published catalogue of "twenty thinking operators"), and **at the meta
level an operator may be an element** (otherwise the operator library stays closed and the system can
never learn a new way of thinking).

Two of the five classes had no statable failure condition until now. Each has one, and it is a
construction rather than a score. **Analogy is a partial functor:** a correspondence that preserves
relations but not their composition is similarity, not a mapping — so its failure condition is a broken
composite, and the quantity that measures it is a composition-preservation rate, not a weighted
systematicity score (§5.5's missing "systematicity weights" are this rate). **Decomposition is a gluing
condition:** pieces that are each consistent on their overlap and still do not assemble have failed for a
reason no extra measurement repairs — a global obstruction, not a missing number. Neither class may be
scored: a composite either survives the correspondence or it does not, and local consistency either glues
or it does not. **[wager]**

Two boundaries decide what may enter the kernel:

1. **Only operations that are exactly decidable on a finite context may enter the kernel.** Similarity,
   vector approximation, and prior scores break the necessity criterion — they make the system look like
   it is thinking while making "the language is insufficient" unprovable.
2. **The library answers "which", we answer "whether and enough".** Retrieval says which candidate looks
   closest; the kernel says whether this set of samples can be expressed at all under the current
   vocabulary, and which predicate is missing. That is a division of labour, not a competition.

Hence: **a unified interface matters more than a unified algorithm.** Outputs of different operator classes
are incommensurable (a similarity score, a mapping score, a candidate program, a verification success);
collapsing them into one "correctness score" is the standard mistake of systems of this kind. The
interface only has to unify what is read and written, what makes the step admissible, when it is void, and
what it costs.

### 3.3 Candidates and search

Keep several explanations alive, across abstraction levels, without fixing an answer at the start. Search is
not the goal: inside a fixed vocabulary it can only rank existing candidates; the real change comes from
the representation class of 3.2.

The division of the three computational tools in one sentence: **vectors make it come to mind (recall),
symbols make it stateable (checkable structure), criteria make the choice right (compression, held-out
prediction, resistance to counterexamples).** None may impersonate another.

The hybrid division is fixed too: **the model proposes with high recall, the structure engine refines, the
verifier excludes, the controller allocates.** Letting one do another's work loses coverage and confidence
at the same time.

### 3.4 Evaluation

One measure: Φ = |L(s)|, the number of hypotheses still alive. It says how many remain, not which is
better.

Complexity, coverage, and evidential support must exist as explicit criteria; they may not be smuggled into
Φ. The moment they are, the theorems no longer apply.

For the same reason, outputs of different operator classes are incommensurable: retrieval gives a
similarity, mapping a systematicity score, induction a candidate program, verification a pass. They cannot
be weighted into one total. Evaluation is stated per class, and the controller trades them off against the
task.

Evaluation also splits in two, and the halves may not impersonate each other: **program-checkable**
(consistency, coverage, executability, a collision disappeared) and **judgement-requiring** (on point,
profound, "that's it"). The first enters closure, the second enters the oracle.

Two companion constraints:

- **Self-assessment is unreliable.** Self-correction without external feedback fails in the experiments and
  sometimes makes answers worse. What can be checked goes to a test, a solver, or a proof checker; what
  cannot is structured into hypotheses, evidence, and state.
- **Simplicity is not truth.** A rule that is too simple drops conditions, and a complex rule can be
  correct; "shortest is true" is false. Compression is one criterion among several.

The two criteria, written out — neither is new; what is new is wiring them to a decidable trigger:

> Choose the candidate: argmin \[L(M) + L(D|M)\]. Choose the next step: argmax E[ΔI|a] / Cost(a).

### 3.5 Control

V(o|S) = E[ΔQ] − λC(o) − μR(o). In engineering, rules first, learning later:

1. A type conflict: check the representation first.
2. Several explanations: look for discriminating evidence first.
3. An executable candidate: run the verification first.
4. The structure cannot explain the phenomenon: extend the hypothesis space.
5. Several steps without improvement: stop or ask for input.

The controller is not an "infinite reflection loop". It manages the cost and benefit of thinking.

The initial schedule (a first policy, not a permanent procedure):

| State | Do first |
|---|---|
| no structure yet | have the model extract relations and types |
| two structures to compare | structure mapping |
| several expressions of the same shape | least general generalisation |
| several cases sharing a rule | rule induction |
| too many candidates | retrieval |
| rules and constraints conflict | verify, then look for a counterexample |
| evidence cannot separate the candidates | acquire evidence, or keep several hypotheses |
| repeated steps with no improvement | stop, change policy, ask a person |

The table should eventually be learned from past tasks: which operator suits which state, how much it
usually gains, which operations tend to produce wasted search.

### 3.6 Memory

Three tables that may not be merged:

| Table | Holds | Admission |
|---|---|---|
| task state | the current problem's nodes, candidates, evidence, history | none |
| operator experience | which operator is adopted more often in which class of state | verified records only |
| reusable abstractions | structures or subroutines recurring across tasks | at least two independent occurrences |

Learning from failure is not learning the right thing. Memory without admission criteria turns one mistake
into a habit used repeatedly.

Three tables are not enough once a task runs past a few rounds, because none of them records what the
vocabulary could not see. A fourth kind of entry, kept apart from the other three: **a coordinate with its
witness.** A coordinate is an executable quantity that separates at least one pair of real candidates and is
unchanged by changes that should not matter; it enters only together with the pair it separates. A witness
enters only after a misjudgment: two candidates were indistinguishable under the then-current vocabulary, one
of them was chosen, and the choice turned out wrong. The pair is the witness; the next coordinate is whatever
measurable quantity separates that pair. Progress over a long task is counted in coordinates, not in lines,
and a word no coordinate separates is reported as a word and deleted or replaced. What this forbids: a
coordinate without a witness, a preference recorded as a coordinate, and a concept that outlives the object
set it was adopted on without being re-checked against the current one. **[engineering]**

### 3.7 Interface

The model is an external service, not the runtime. The protocol is a file: request (state summary,
permitted actions, budget) → response (candidates, reasons, optionally a question). Validation is a pure
function, and a refusal states its reason (wrong protocol, out of range, duplicate name, duplicate ids).

Judgement has three values: **yes, no, evidence insufficient**. The interface must be able to express the
third, or the system is forced to take a position when it has no evidence.

The shape of the protocol deserves its own sentence: it wants **options with confidence**, not an essay. An
interface that can only say "I think…" cannot be validated; one that returns candidates, reasons, and an
optional question can. This is not a style preference; it is the only way to connect a language model to a
decidable procedure.

Three non-negotiables:

1. A response may not write the state directly.
2. A response that fails validation is not adopted, but it is recorded.
3. A non-adoptable product (a conflict report, for instance) never enters the adoption path.

### 3.8 Record

One line per offer, not only per adopted proposal; it carries the budget, a state fingerprint, and the
reason for stopping. The record must replay, and the fingerprint must cover every state bit.

The record is not a log. It is the controller's training set and the only carrier of "what this failure
told us".

## 4. Eleven laws

In force when writing code. Each states what it forbids and what currently checks it.

| | Law | It forbids | Current check |
|---|---|---|---|
| L1 | The state is the only truth; text is I/O | treating "thinking text" as the state | operators receive a state snapshot |
| L2 | An operator carries type, warrant, falsifier, cost | an operator with no failure condition | four-method operator interface and component tests |
| L3 | One write path: propose → validate → adopt | a component writing the state | the assertion that the probe report is `(not adopted)` |
| L4 | Dependencies point one way: kernel has none, capability lives in components | the runtime depending on retrieval or analogy crates | two `cargo tree` assertions |
| L5 | It must be able to stop, and record why | a loop with no stopping condition | the controller's "nothing helps, stop" test |
| L6 | The record replays, the fingerprint covers the state | a path that writes no record | round-trip and fingerprint tests |
| L7 | Budget is a first-class citizen | an offer without a cost | cost estimation is a required interface method |
| L8 | An operator with no scenario and no number is not delivered | an interface with no scenario | the components gate runs three proposers |
| L9 | Docs are claims; a claim carries a command | a repository fact with no command | the claims gate: PASS / FAIL / NEED |
| L10 | One falsifiable increment per round | three untested ideas in one round | the results file gains one numbered conclusion |
| L11 | Invention is admitted with a held-out prediction | adopting a predicate because it separates the pair that motivated it | the validator refuses a candidate whose warrant names no unmeasured object |

L2 and L9 are what separates this from architecture prose: one requires every step to be able to fail, the
other requires every sentence to be able to be refuted.

L11 is what makes L2 load-bearing rather than decorative. The kernel's own result is that a Boolean
recombination inside the vocabulary cannot break a collision; a predicate that does break one is therefore
not an invariant of the current representation. **Invention is always symmetry-breaking, and the capacity
bound prices consistency, never the break.** So a candidate must name an object it was not built from and
predict that object's label before the label is measured. A concept and a lookup table have the same
extension on the objects they were built from; only the unmeasured object tells them apart. What this
forbids: adoption on fit alone, and a warrant that is printed but never used to decide. **[theorem + engineering]**

A green gate is not evidence that the change is the one intended. The gates check that the repository still
behaves; they cannot check that the diff is the change meant, and a commit whose message describes three
edits while its diff contains twelve is a lie the gates pass happily. Read the diff before writing the
message, and treat a mismatch between the two as a defect of the commit, not of the gate. **[engineering]**

## 5. Theory map

One sentence first: **the contribution is not an algorithm, it is the trigger conditions and the typing
discipline** — when to call which operator, how the state changes afterwards, and what counts as decided.
The algorithms can come from elsewhere; these three have no off-the-shelf source.

### 5.1 What each theory fixes

| Theory | Fixes | Can do | Cannot do |
|---|---|---|---|
| formal concept analysis | the attribute vocabulary | concept lattices, attribute exploration (the one mature question-driven vocabulary expansion) | an intension outside the vocabulary |
| inductive logic programming | the predicate signature | induce rules from examples and background | crossing the signature needs predicate invention, costly and without a criterion |
| structure mapping | the relation vocabulary | analogy, candidate inferences, systematicity | create a new relation type |
| vector symbolic architectures | the encoding and dimension | approximate retrieval, binding, cheap recall | similarity is not equivalence; retrieval is not judgement |
| minimum description length | the description language | compare models inside a given language | say which language to switch to |
| Bayesian inference | the hypothesis space | manage uncertainty once the space is given | decide where the space comes from |
| active learning | the space and the cost model | choose the next question | with a fixed space the gain is capped (measured here: close to random) |
| abstraction refinement | the abstract domain | refine the abstraction from counterexamples | the abstract domain itself is supplied |
| program induction | the DSL | compress a corpus, grow a subroutine library | restructuring is driven by the corpus, not by one query |
| category theory | morphisms and composition | describe structure mappings and what they preserve | produce hypotheses |
| causal representation learning | interventions | separate commonality from mechanism | needs an environment that can be intervened on |

### 5.2 The vacancy

No existing system does three things at once: **expand the vocabulary because of a question, use natural
language predicates, and decide membership with a cheap oracle.**

Two exceptions come closest: attribute exploration (question-driven vocabulary growth, but the attributes
are supplied and every question is answered by a person) and the discovery of structural form (a Bayesian
choice among trees, chains, rings and orders, but driven by a whole data set rather than by one query).

The vacancy is their composite: a predicate is proposed by a model, membership is judged by a person or a
cheap oracle with one word, and the next question is chosen by information gain. The slot is narrow, but it
is real, and small enough to build something measurable in weeks.

The literature has to be stated precisely: a model attached to a **single** symbolic tool (a solver, an
inducer, an interpreter) has reproducible gains; a model attached to a **full set** of structural operators
and scheduled by a controller has no verified precedent. That says the work is not redundant — and that
there is no roadmap to copy.

### 5.3 Abandoned routes and refuted practices

**Three routes:**

1. **Re-implementing an LLM with classical cognitive algorithms.** It lacks the training scale and the
   data, and answers no new question.
2. **More elaborate prompts.** A prompt tells the model what to consider now; an engine must be able to
   operate on the state, observe the result, judge progress, and find the missing structure.
3. **Simulating the brain.** The brain also stops at the first plausible explanation (working memory holds
   about four chunks and leans on external scaffolding to carry long-range constraints). Aiming at
   "brain-like" copies the bugs along with the abilities. The goal is an **auditable scaffold**.

**Two practices the experiments refuted:**

4. **Adaptive thresholds / a Bayesian gate.** Tried; both lost to a fixed threshold derived directly from
   the error rate. Lesson: tuning a prior is not automatically better, and a mechanism that a constant can
   beat does not belong in the architecture.
5. **Treating "think again" as progress.** Closure is idempotent and evidence is the only class that
   strictly lowers Φ; repeating inference over the same evidence changes nothing.

**One discipline:** CEGAR, DreamCoder, and program induction exist here as motivation, not as
implementations. Each fixes something (an abstract domain, a DSL, a corpus), and what this project decides
is exactly whether that thing should be replaced.

### 5.4 Why LLMs succeeded, and how the system divides labour

Not because those frameworks were wrong, but because they lacked most of these five:

1. A learnable objective that scales (next-token prediction, with the whole internet as supervision).
2. Representation and capability learned together (classical frameworks need the representation defined
   first).
3. One unified learning path (not a collection of local capabilities).
4. A cheap and powerful candidate generator (no more hand-enumerated search space).
5. An industrial base (compute, data, engineering, distribution).

So the position is not "re-implement an LLM with classical algorithms" but a division into three layers:

| Layer | Who | Responsible for |
|---|---|---|
| learning and generation | the model | understand input, propose candidate structures, hypotheses, explanations, programs |
| explicit cognitive computation | structure mapping, rule induction, retrieval, verification | execute defined operations on structure, find common patterns, check constraints, produce counterexamples |
| meta-reasoning and control | the metaform runtime | decide what to do next, when to stop, how much to pay |

Two corollaries:

1. **The big model does the big thing; we do the small thing.** We do not train a model and do not compete
   with it on knowledge; we build what it cannot: explicit state, a decidable trigger, cost and stopping.
2. **Success at one capability does not compose into a system.** Having an induction algorithm is not
   knowing when to induce; having a verifier is not knowing what to verify. Coordination is the research
   object, not a by-product of wiring.

Merging the repositories into one monorepo solves a software-organisation problem; it does not produce a
paradigm-level intelligence.

The model compressed a great deal of cognition into one network, but did not remove the need for external
structure: it keeps no state across turns, does not change its own representation, and cannot say under a
given budget why it stopped. Those three are the runtime's work.

Existing neuro-symbolic work supports "attach a solver or an inducer in a constrained domain and it does
better", but no system treats "**these examples cannot be expressed in the current representation**" as a
decision to make. That is this project's vacancy (§5.2).

### 5.5 The capability list and where each item lives

The opening list — association, induction, abstraction, analogy, decomposition, composition,
counterexample, planning and inverse planning, intent inference, meta-learning, decision, causation — is
not twelve parallel modules; each sits somewhere different. Account for them one by one and it becomes
clear what is actually built and what is only a name:

| Capability | Position in the system | Now | Missing |
|---|---|---|---|
| association (recall) | vector retrieval, candidate generation | a component, empirically weak here | better contrastive encoding and a recall criterion |
| induction | generation + representation | yes | no criterion for predicate invention |
| abstraction | representation (least general generalisation) | partial: the operation itself is missing | anti-unification, then types and levels (§6.7) |
| analogy | structure mapping | yes | the composition-preservation rate defined in §3.2, in place of a weight |
| decomposition | goal rewriting | untyped | a goal algebra, or the scheduler cannot reason about it |
| composition | generation | partial | recombination must not break existing distinctions (a theorem supports this) |
| counterexample | evidence | yes | noise tolerance bound |
| verification | closure | yes | only for executable candidates |
| planning / inverse planning | none | not done | needs an action model and a utility; outside the decidable domain for now |
| intent inference | none | not done | needs a model of the other; the success criterion is undecidable |
| meta-learning | control + experience table | skeleton | where the policy comes from and how it is verified |
| decision | control | yes | a cost model |
| causation | none | not done | needs an environment that can be intervened on |

How to use the table: any claim that "we can do X too" first finds X's position here and reads the "now"
column. No position means not done.

## 6. Route

Two numberings answer two different questions: M says how far we have got; R says how far the system can
change itself.

### 6.1 Delivery milestones

| | Milestone | Criterion | Now |
|---|---|---|---|
| M0 | a runtime exists | state, operators, controller, record — all four, running one scenario | done |
| M1 | a representation change can be triggered | the engine decides "vocabulary insufficient", gives a coverage target, accepts an external predicate | done: judgement and coverage target work; the protocol accepts a defined predicate (`define: {all_of\|any_of}` over existing predicates), and `state add --define` stores the definition so the extension is recomputed on load when the object set changes |
| M2 | real use at equal budget | a fixed model and budget, four arms, conclusions from unseen tasks | pilot: two arms, four cases each, budget not strictly accounted |
| M3 | accumulation across tasks | the experience table drives selection, proposals per task family fall, every policy comes from a verified entry | skeleton: the table exists, and the controller consults it through the experience hint, which changes ask order only and never the adopted proposal |
| M4 | bootstrapping | metaform iterates metaform, and every deliverable carries an executable claim | embryonic: the claims register and the gates |

### 6.2 Self-improvement levels

R0 external assistance: the model calls fixed operators; the system cannot change its own procedure.
R1 strategy adaptation: operators are chosen by task, history, and cost, and the schedule gets better.
R2 mechanism improvement: the system finds a defect in its rules, search procedure, or representation,
generates a new version, and judges it by independent tests.
R3 improving the improver: it can also change how rules are generated, experiments are chosen, and
improvements are evaluated.
R4 open-ended expansion: across many new task types it repeatedly finds and verifies new representations,
algorithms, and strategies, and the gains accumulate.

The criterion must be an experiment, not self-assessment: test on tasks that took no part in the change,
compare with the previous version, and check whether it merely memorised the test set, spent more compute,
or traded other abilities for one metric.

### 6.3 Where we are

Honest position: **M0 and M1 done, M2 pilot only, M3 a skeleton, M4 embryonic.**

What can be said now:

- The insufficiency decision is provable and compiles with the repository (Theorems 1–11, 13 declarations,
  no warnings).
- An object question pays about as much as a random question that still carries information; the gain
  comes from *the label that was chosen*, not from the proposal machinery.
- On real-defect probes a static module feature suffices; failures inside one module do not need the loop,
  only cross-module failures do.
- Vector retrieval is at or below chance on the three families here (0.1 against 0.2 at k=2, 0 on k3 and on
  the noisy family); but an earlier measurement with twelve objects and the full attribute pool gave 0.81.
  The component is not useless in general — it is useless *here*, and that boundary is exactly what the
  protocol exists to expose. Negative results are written down, not hidden.

What cannot be said yet:

- The loop has run against one real model, and the effects are small. A 3.8B local proposer
  (Phi-4-mini Q4_K_M) improves recovery by 15--20 points over placebo on the witness-pair message
  (58% against 39%, p = 0.007, pooled over runs); a single nineteen-instance batch spans 32--68%; a
  six-step chain produced a +3 point gap against a pre-registered 15 point bar. Below a capability
  floor the witness and the count are the same number, so the failure mode the protocol exists to
  catch is *unmeasured* there, not absent. See `experiments/RESULTS.md` sections 29--31.
- The tree-shaped fragment of the relational necessity criterion is decided by colour refinement with a minimised witness; general conjunctive queries (a homomorphism test) are still open.
- There is no validation on a large real repository.

### 6.4 The next gate

**Regressions on a real repository.** Take 10–20 real fixes, use "fails before the fix, passes after" as
ground truth, derive features from the dependency graph and test metadata, and run four arms at equal
budget: model alone; the model with ordinary tools; fixed toolchain; dynamic scheduling.

The core hypothesis, written so that it can be refuted:

> Hold the base model fixed, externalise part of the reasoning into an explicit state, use composable
> structural operations and verification, and let a controller choose the next step — does that improve
> structure discovery, hypothesis quality, error correction, and transfer across tasks at equal compute
> budget?

The criterion is not "we won". It is: **the loop produces a measurable gain only on cross-module failures.**
If a static feature suffices inside a module, that is a result too — and a more valuable one.

Six parts of the protocol; a conclusion does not stand without all six:

1. **Four arms present**: model alone; the model with ordinary tools (read files, search the tree, run the
   tests, read the compiler); fixed toolchain; dynamic scheduling. Beating the bare model is expected and is
   not evidence. The arm that decides this gate is the tool-using model, because it is the strongest
   baseline available without this engine; a gain measured only against the bare model is not a result.
2. **Matched budget**: tokens, calls, and time are all accounted; a gain bought by calling the model more
   often does not count.
3. **Ablation**: removing structure mapping, rule induction, counterexample feedback, and dynamic
   scheduling must each show a drop, or the cause is unknown.
4. **Unseen tasks and transfer**: conclusions come from tasks that took no part in the design, and survive
   a change of wording.
5. **Negative results allowed**: negative findings are written up; reporting only the successful half is
   not an experiment.
6. **Independence.** Any experiment run through the protocol must satisfy three conditions, each learned by
   breaking it: the request must not contain the answer (it carries the evidence, and the evidence ids *are*
   the target extension, so a proposer can echo them); no published atom may equal the withheld set, nor may a
   union or difference of two atoms reproduce it (a held-out split leaks through the vocabulary); and the
   proposer must be independent of the labels (the same agent that chose the labels is a demonstration, not a
   measurement). A request-shaped experiment that fails any of these measures copying, not discovery.

Read other people's results by the same list: fair baseline, matched budget, independent test set,
sufficient ablation, transfer tested, failures reported, reproducible. **Peer review is not the same as a
result that holds.**

### 6.5 Absorption policy

Do not reinvent, and do not pretend to absorb.

1. **Check first.** If a mature implementation exists, depend on it, add the crate, leave its core alone.
2. **Then re-create.** When nothing usable exists, the licence allows it, the core is small, and the
   interface is clear, re-create the core and state the provenance.
3. **Rewrite last.** A rewrite redefines the problem; it is not the easy path. Rewrite only when even
   "what does it fix" cannot be stated.
4. **One line per ported piece**: what it fixes, therefore what it cannot do, where our interface is.
5. **Stop-loss.** A component that has produced no scenario and no number within two rounds is reverted to
   a dependency, or deleted.

Three hard constraints:

- **The three-way split.** Licence-permitting ports (MIT and the like); no licence means a clean-room
  rewrite; too heavy a dependency means an interface only, not a place in the tree.
- **A licence permitting it is not the architecture permitting it.** A zero-dependency kernel is an
  architectural constraint and does not bend because a library is convenient.
- **One admission criterion.** On cases the component was not tuned on, its judgement must improve;
  otherwise it stays "available, off by default". Cost is recorded and reported but never decisive:
  token prices fall, so a component admitted for being cheaper is admitted for a property of the
  market, not of the component.

One measured fact supports the caution: the Rust ecosystem has usable pieces for vector symbols, rule
learning, and Prolog, but nothing for structure mapping; so the clean rewrite priority falls exactly on
"structure mapping plus a licence-clean rule inducer" — the two genuinely fresh pieces in this repository.

Four engineering disciplines:

- Ported MIT code is credited in NOTICE and keeps its copyright header; code with no licence cannot be
  copied (ideas are not copyrightable, code is).
- No FFI. A single binary with no external dependency is part of the product, not a compromise.
- Every port carries a differential test against the reference implementation; otherwise "absorbed" is
  self-reported.
- Performance targets are set before measuring: retrieval over 10⁵ candidates under 10 ms, structure
  matching over 10³ nodes under 100 ms, coverage testing over 10³ examples under 1 s. Miss them and do not
  optimise yet.

### 6.6 Definition of done for a round

1. It builds, and every gate is green.
2. The claims register is updated: a new fact has a command; a fact without one goes to NEED.
3. The results file gains one numbered conclusion, or says explicitly that the round has none.
4. The English document and the Chinese copy are in sync.
5. One falsifiable increment, not three ideas.

### 6.7 Component capability, and what each library actually sells

A component is admitted for a capability, not for being well known. The question is never "is this
library good" but "which row of §5.5 does it fill, and at what price". Read this table before adding
a dependency or porting code.

| Library | What it actually implements | Weight and licence | Evidence we hold | Verdict |
|---|---|---|---|---|
| torchhd | the full HDC operator set: bind, bundle, permute, associative memory, classifiers | ~24k lines, Python on PyTorch, MIT | our VSA retrieval is at or below chance on three families here (0.1 against 0.2 at k=2); capacity falls from 1.000 to 0.094 clean at 1024 pairs | reference for semantics only; do not port more operators |
| hypervector | encode and bundle over FHRR angles; no unbind, no memory | ~1.9k lines, Rust, MIT | read and rejected earlier: the missing pieces are the ones that matter | rejected |
| Cognitive-Rule-Engine | anti-unification with a normalisation choice and a score, plus a rule engine | ~31k lines, Python, MIT | our abstraction row is "partial"; the least general generalisation is exactly what is missing | port the anti-unify idea (mask copy), no dependency |
| metagol | meta-interpretive learning over a compact Prolog | ~2.5k lines, Prolog, BSD-3 | needs a Prolog runtime; its search is over a supplied language bias | reference only |
| Popper | the strongest current ILP search | very large tree, needs SWI-Prolog, MIT | our theta-subsumption covers the decidable slice; a stronger search under a wrong vocabulary is more confidently wrong | reference only |
| conexp-clj | attribute exploration with the Duquenne-Guigues basis | ~19k lines, Clojure | our `explore` and `answer` now close the same loop | reference for completeness checks |
| SME-clj, ANASIME | small readable structure-mapping implementations | ~1k lines each, Clojure and Python | our SME passes the golden geometry; the open question is systematicity, now stated as a composition-preservation rate (§3.2) | reference for differential tests |
| scryer-prolog | embeddable Prolog in Rust | ~96k lines, Rust, BSD-3 | not used yet; the only candidate if rules must run in one binary | candidate, not admitted |
| egg | e-graph equality saturation with explanations | ~11k lines, Rust, MIT, one git dependency | its equality is over terms; ours is over extensions, and the mask is already a complete normal form | rejected |

Two rules follow from the table and are binding:

- **A capability with no measured gain is not a capability.** The VSA operator set is complete and
  still useless here; adding the rest of the library would not change the measurement, because the
  failure is representational, not arithmetic.
- **A port is justified by a missing row, not by a missing function.** Anti-unification fills the
  abstraction row; a faster bind does not fill anything.

Roadmap, in the order the table implies:

1. **R0, done.** Kernel, protocol, held-out prediction, discriminating rival, coverage table, the gate set.
2. **R1, now.** Fill the two rows the table says are thin: port anti-unification and state the
   composition-preservation rate for analogy. Neither needs a dependency; both need a test.
3. **R2.** Re-decide VSA on one experiment: contrastive encoding against the raw-trace fallback on the
   same three families. If it does not beat the fallback, demote it to "available, off by default"
   and stop spending on it.
4. **R3, blocked.** Cost-matched, cross-module real tasks. The protocol for them exists; the metered
   model access does not, and a run without a budget number is not a comparison.

### 6.8 Breadth as an enumerated version space **[wager]**

This subsection is speculative and lives on a branch until something in it is measured. It answers a
narrowness that no amount of search fixes: asked to explain a case, a model produces one reading and
does not report the others. Formally it returns a point in the space of readings consistent with the
evidence, and never the space itself.

Name the space. Given objects and the current vocabulary, the **version space** is every predicate
consistent with the labels so far. Three quantities are computable today and none of them is produced
by the model:

- its size, which falls as labels arrive;
- the colliding pairs, which are exactly the readings the current vocabulary has collapsed;
- the smallest set of objects whose labels would halve it, which is the question to ask next.

Different directions, angles and levels are not inspiration either; they are a product. Each axis
below is a transformation of the reading, and the grid is finite:

| Axis | Values | The question it opens |
|---|---|---|
| scope | one case / a class / the population | is this an instance or a rule |
| granularity | token / clause / document / system | at which level does the failure live |
| causal direction | cause / effect / common cause | is A causing B, or is C causing both |
| time | state / trend / cycle | is this a moment or a process |
| role | actor / observer / adversary | whose objective is being optimised |
| criterion | correctness / cost / novelty / risk | by what is this judged good |

Six axes and three values each is eighteen readings. The engine's job is not to invent them but to
classify them: which readings are extensionally identical given the evidence (the angle did not
actually change), which contradict a label (discard), and which remain alive (those are the other
possibilities).

Three numbers make "fuzzy to clear" measurable: the size of the version space after k labels, the
number of the eighteen readings that collapse into one, and the number of times the engine has to say
the vocabulary is insufficient -- that is, the number of new criteria the task genuinely needs.

What this does not do: it does not add knowledge, and it does not rank the surviving readings. The
axis grid is a checklist, and a checklist can be wrong about which axes matter. The wager is that a
finite, explicit grid plus a measured version space beats an unbounded request to "think of other
angles", because the former can be audited and the latter cannot.

### 6.9 What "smarter" is measured with

The field already has settled metrics; do not invent them. Three tiers, separated by what they cost
to compute:

| Tier | Needs | Metrics |
|---|---|---|
| no labels | repeated independent judgements | paraphrase flip rate, self-consistency, semantic entropy, inter-judge alpha |
| about a hundred labels | ground truth | risk-coverage and AURC, selective accuracy with intervals, the 3x3 confusion, conformal coverage |
| a purpose-built suite | fifty to a hundred items | interpretation coverage, clarification quality -- did it ask the question that mattered |

**Two protocol actions are missing, and without them the headline metric does not exist.** A candidate
must be able to answer `unknown`, and every answer must carry a `score` that orders abstention.
Without both, the coverage axis is empty and AURC is the average error of an arbitrary ordering --
which is what the six-question batch measured: every arm had an unknown rate of zero, and the
invention arm scored a *lower* AURC than the engine arm while being less accurate.

**Not used here**: pass@k or pass^k on a single verdict, which reward luck; ECE as a headline, which
is binning-dependent and unstable at this sample size; exact match or token F1 as evidence of clarity.

**Numbers we have**, item level, 72 items per arm: the engine arm answers 0.972 correctly with an
interval of [0.904, 0.992], the invention arm 0.958, the no-engine arm 1.000, all with zero
abstention. The reading is that on that batch of decidable questions the baseline did not lose.

Cost is handled by §6.5: recorded and reported, never decisive.

### 6.10 Closed world and open world **[wager]**

A user describes a project in a vague paragraph, often as a list: "combine the abilities of these
libraries, solve unclear goals, stop the AI from only using the words I already said". Three
complaints recur in such a paragraph and they are one complaint:

1. it thinks only on the elements I enumerated;
2. it cannot reach what the context does not contain;
3. it cannot see the problems in its own reasoning.

The common cause is a closed-world assumption. A person's enumeration is a sample drawn from a set
they have not stated; the model treats the enumeration as the set. The complement is then empty, and
with an empty complement there is nothing to be surprised by.

Formally: given stated elements E and a vocabulary V over objects O, the engine can compute

- the **complement coverage**: which of the axis-generated readings (§6.8) have an extension not
  equal to any element of E -- those are candidates for "what I did not say";
- the **axis equivalence rate**: which of those readings have an extension identical to a stated
  element, that is, the angle changed the words but not the distinctions;
- the **bet divergence rate**: of the predictions recorded, how many were placed where two rules
  disagree (the `rival` requirement in §4, L11). A prediction on which every candidate agrees is a
  prediction that could not have failed, and it is counted as zero information.

Three numbers, all computable without a model, and all descending as the conversation becomes more
specific: a high complement coverage means the list was not the universe; a high equivalence rate
means the "new angles" were rewordings; a low divergence rate means the bets were decorative.

The third complaint is not a metric but a boundary. No checker can answer "is this the
formalisation I meant": Lean cannot say whether the theorem is the one you wanted, the compiler
cannot say whether the program is the one you wanted, and this engine cannot say whether the
predicate is the one you wanted to invent. That layer is closed to every tool, which is why a
person's one-word answer is a scarce input rather than an interaction-design choice.

What this does not do: it does not add knowledge, and it cannot see outside the evidence. It can
only make the difference between "stated" and "unstated" visible, and price the bets that cross it.

### 6.11 The engine is the first example of what it asks for

If a project is meant to build good architecture with this engine in the loop, the engine has to be
one. The claim is not taste; it is a set of properties that can be checked.

Four tiers, and an import may only point inward:

| Tier | Holds | May use |
|---|---|---|
| kernel | types and pure decisions: masks, state, protocol validation, metrics arithmetic | nothing above it |
| runtime | operators, controller, records: suggest, explore, apply, answer, laws, predictions, plan | kernel |
| interface | CLI, MCP, report rendering | kernel and runtime |
| components | capability crates: VSA, SME, ILP | kernel types only |

`metaform check layers` enforces the direction over the engine's files and fails on a module with
no declared tier, so adding a file forces the author to place it. That is the only kind of layering
rule that survives: the one that turns red.

Two further properties follow from the same principle, and they are about maturity rather than
minimalism:

- **A facility or an entity, never both.** A component that owns a queue and also exposes synchronous
  calls will be used both ways and will deadlock in one of them. Allocation and formatting are
  facilities; schedulers, mailboxes and drivers are entities.
- **Nothing enters the tree as a prototype.** A capability ships with its tests, its gate, its
  documentation in both languages, and a claim row if a reader can re-run it. A sketch that only
  works on the author's machine is not a smaller version of the feature; it is a different thing
  that will be defended later.

What this forbids: an import that points outward, a module with no tier, a component that is both a
facility and an entity, and a feature whose only evidence is that it ran once.

## 7. Boundary

### 7.1 What it is not

| Not | Why |
|---|---|
| a claim of AGI, RSI, or SI | a capability level is judged by independent experiments, not deduced from an architecture |
| consciousness | cognitive performance does not prove subjective experience |
| a new model | it does not compete with the model on knowledge |
| a prompt framework | a prompt changes what is considered; an engine changes what can be done to the state |
| a re-implementation of classical algorithms | only when a Rust implementation is missing and the core is small |
| classical systems packaged into one algorithm collection | software organisation is not a cognitive architecture; adding modules does not produce coordination |
| a brain simulation | it copies the bugs, not the ability |

The three levels do not imply one another either: AGI need not have RSI; a local RSI need not be an AGI;
RSI does not guarantee SI. Any claim that "we are at R2" must be supported by a controlled experiment on
tasks that took no part in the change.

### 7.2 What it can promise

Three decidable assertions, and only three:

1. Under the current vocabulary, is this set of samples definable; if not, which pair is indistinguishable.
2. To separate them, which predicate must be added, and which pairs it must cover.
3. Which object to ask next (the minimum number of questions after merging equivalence classes).

Every other output — explanation, analogy, insight — stays out of the evaluation.

It is a meter, not a second mind, and its one act is to install a refusal in a medium that has none.

### 7.3 What it cannot promise

- It does not invent predicates. It decides that invention is required and gives the coverage target; the
  predicate comes from outside.
- It does not infer facts from structure. Without evidence, a structural possibility is only a candidate.
- It does not guarantee a hit. It can only raise the hit rate per unit cost.

## 8. How it dies

| Failure mode | Symptom | Early signal | Countermeasure |
|---|---|---|---|
| promptification | conclusions come from wording, not computation | the gain vanishes when the model changes | P6's experiment must separate the two |
| dashboard | rich state, unchanged decisions | adoption rates of all operators are similar | the controller must be able to stop, and say why |
| rubber stamp | validation always passes | adoption rate near 1 | keep the distribution of refusal reasons, review it |
| paper machine | claims grow, evidence does not | the NEED list grows | clear at least one NEED per round |
| formalism as decoration | beautiful definitions, no computation | a theorem with no operator | every theorem maps to an operator class |
| randomised controller | choices unrelated to cost | changing the budget does not change the policy | the cost model goes into a test |
| bootstrap illusion | the tool proves the tool | conclusions hold only on the project's own demo | external tasks and unseen data |
| overfitting the benchmark | good numbers, zero transfer | breaks on the next domain | change the domain every quarter |

## 9. Terms

| Chinese | English | Exact meaning here |
|---|---|---|
| 内涵 | intension | the rule that generates a set; represented as predicates or programs |
| 外延 | extension | the members of a set; samples are part of an extension |
| 表征 | representation | the vocabulary and structures currently expressible; changing it is not changing the answer |
| 活集 | live set | candidate intensions consistent with all evidence |
| Φ | potential | the size of the live set; the only measure |
| 算子 | operator | a state transition with type, warrant, falsifier, cost |
| 提议 | proposal | a candidate from outside; it may not enter the state unvalidated |
| 采纳 | adopt | validated and written into the state; not the same as accepted |
| 缺口 | gap | what the current structure cannot explain; the controller's target |
| 命中 | hit | the external judgement "that's it"; unconfirmable internally |
| 样本 | sample | the examples the asker gives; not the object he wants |
| 可判定 | decidable | a program returns 0/1 in finitely many steps |

## 10. The three tools, and a self-check

**VSA recalls, structure mapping aligns, rule induction states a rule, and the kernel alone can say
"the vocabulary is insufficient".** This is not a pipeline: the loop is kernel-judges → ILP/SME
propose → the controller allocates by pairs separated per cost, and VSA only narrows the candidates.

**VSA.** Objects, roles, and fillers become bipolar vectors; binding composes, bundling superposes, and
a query is a contrast between two objects, not one vector. The output is a ranked shortlist — the table
is the whole promise (top-1, D = 4096, 64 probes):

| pairs | clean | 10% noise |
|---|---|---|
| 64 | 1.000 | 1.000 |
| 256 | 0.906 | 0.625 |
| 1024 | 0.094 | 0.047 |

Guarantees noisy recall, deduplication, and fast screening of a pool too large to enumerate — never
logic or equivalence: a cosine orders candidates, it never judges them.

**SME.** Structure mapping aligns two relational systems by their largest consistent match.
Correspondences are proposed expression by expression, only between same-functor expressions; entity
correspondences arise only from matched argument positions — never entity-to-entity proposals.
Guarantees structural correspondences and transferable candidate inferences; not that either is true.

**ILP.** Rule induction searches clauses in a fixed signature; every result has the shape of
`grandparent(X,Z) :- parent(X,Y), parent(Y,Z).` — positive coverage, no negative. The relational
criterion: two red objects, one linking to green, one to blue, are separated by `link(X,Y), green(Y)`;
two linking to green are bisimilar, and no tree-shaped pattern separates them. Guarantees an explicit,
executable, checkable rule; not truth — coverage is consistency; held-out data is the guard.

The single status authority is §6.1; this table is the same information viewed by part.

| remaining part | status today, not the plan |
|---|---|
| kernel (Φ, collision test, `INSUFFICIENT`) | exists; Theorems 1–11 compile in Lean |
| controller | exists; adopts by pairs separated per cost, cheapest on a tie |
| record and experience | exists; consulted through the experience hint, which changes ask order only |
| predicate layer | accepts `define: {all_of\|any_of}` over existing predicates; a definition is constant on the request vocabulary's equivalence classes |
| FCA / attribute exploration | decides which predicate to add; not wired |
| LLM adapter | exists (`experiments/local_proposer.py`); run against a 3.8B local model, whose capability ceiling bounds what the numbers can show |
| CEGAR, MDL | motivation, and a criterion stated but not wired |

**Four confusions.**

1. **More operators is not more intelligence.** Closure is idempotent, generation is monotone, and only
   evidence strictly lowers Φ.
2. **Similar vectors is not the same meaning.** Retrieval orders candidates; it fixes no denotation.
3. **Gluing libraries is not a system.** Coordination is the object of study; all three feed candidate
   intensions, and one measure scores them.
4. **A rule that covers the examples is not a true rule.** Coverage is consistency with samples that
   were not chosen to refute it.

**Self-check.** Three questions before any step: does it need truth or only resemblance; is there an
oracle that can judge it; must it scale to 10³–10⁵ items? The three answers choose the tool —
resemblance at that scale is VSA, correspondences between two structures are alignment, and a rule that
must hold beyond its samples needs an oracle to check it.

## 11. Sources and change

- Sources: three research conversations — the first on code aesthetics and parallel elements, the second
  on A and S and the structure of thought, the third on an external tool and computability — followed by
  the discussion of trade-offs, absorption, and route. The conversations are input, not deliverables, and
  are not tracked here.
- Division of labour with the thesis notes (kept outside the history for now): this file states stances
  and prohibitions; those state the falsifiable research questions and the three competing routes.
- Change rules: changing a claim changes its falsifier in the same commit; deleting a law requires a commit
  message that says why it no longer holds.
