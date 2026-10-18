# 11 The Unified Operator Framework: What I Misunderstood Before

## 1. The misunderstanding

I read the source conversation as: "find the missing predicate from A/B/C, then patch it in with set cover". So I built
`metaform`: an engine for **deciding vocabulary insufficiency + adding words by coverage**.

That is **one operator** (induction within a given language, plus expanding the language when it is
insufficient), not "a general structure of thought".

What you want is: **a unified carrier for several thinking operators** — deduction, induction, abduction,
analogy, plus meta-learning, inductive bias, compositional generalization, inverse design, and evolvable
architectures. These are not different parameters of one kind of reasoning, but **different forms of
judgment**.

## 2. The minimal unified structure

There is no need to build a type theory first. You can fix a four-tuple first and have every operator fill
it in:

```
judgment  Γ ⊢ J          Γ is the context (objects, rules, hypotheses, mappings)
                         J is the form of judgment (there are several, not one)

step      (Γ, J) ⟶ (Γ', J')   carrying three things:
          warrant    on what grounds (proof / coverage / explanatory power / structural correspondence)
          cost       how much was spent (queries, tokens, experiments, time)
          falsifier  under what conditions this step is void (counterexample / better explanation / dissimilarity)
```

**The key point**: `warrant` is explicit and checkable; `falsifier` is explicit and triggerable. Without
these two, an operator degenerates into "whatever the model says".

This is in fact relaxing the type-theoretic `Γ ⊢ t : A` into "multiple forms of judgment":

- type = form of judgment;
- term = warrant (proof, coverage certificate, explanation, mapping);
- checking = verifying that the warrant holds;
- failure = no warrant can be constructed, or a falsifier hits.

## 3. Four operators filled into this table

| Operator | Input judgment | Output judgment | warrant | falsifier |
|---|---|---|---|---|
| Deduction | rules + facts | necessary conclusion | derivation chain (machine-checkable) | a rule or a fact is wrong |
| Induction | set of instances | general rule | coverage + simplicity (MDL) | one counterexample |
| Abduction | observation | set of hypotheses | explanatory power (can derive the observation) | a better explanation was not ruled out |
| Analogy | source-domain structure + target-domain constraints | structural mapping | relational correspondence (SME-style) | the mapping breaks a constraint |

The four operators have **different cost structures**: deduction is nearly free, induction needs samples,
abduction needs experiments, analogy needs retrieval. But **the shape of control is the same**: every step
must pay a warrant, mark a falsifier, and record a cost.

## 4. Which layer the other "modes of thought" change

| Name | What it moves |
|---|---|
| Meta-learning | **the rules themselves**: from "using rules" to "learning rules" |
| Inductive bias | **the rule space**: restricting which rules may be proposed (the hypothesis class) |
| Compositional generalization | **rule composition**: whether existing rules can be substituted, nested, reused |
| Inverse design | **rule inversion**: given a target output, back out the inputs/parameters |
| Evolvable architecture | **rule-set rewriting**: rules can be added, deleted, changed, and which ones to keep can be selected |

These five are not new operators but **operations on the operator layer** (second order). So a unified
framework must allow: rules are data that operators can take as input; that is, `Γ` must be able to hold
"rules" themselves.

## 5. Four conditions "unification" must meet (operational criteria)

1. **Shared context**: one Γ holds objects, rules, hypotheses, and mappings at once, and operators read and
   write the same copy.
2. **Shared cost**: the costs of different operators are addable and comparable (otherwise they cannot be
   scheduled).
3. **Shared failure semantics**: after a falsifier fires, the rule for which step to roll back to is the
   same across operators.
4. **Composable**: A's output judgment can serve as B's input judgment (abduction produces hypotheses →
   deduction derives consequences → experiment adjudicates).

Without these four, "unification" is just collecting the words into one directory.

## 6. Where the current metaform stands

It is **one cell** in this table:

- form of judgment: `definable / undefinable` (one kind);
- operator: induction within a given language + language expansion;
- warrant: a coverage certificate (every positive–negative pair is separated by some predicate);
- falsifier: a collision pair (a new predicate excludes an already-confirmed positive example);
- cost: `r/q + rounds`.

It got three reusable things right: **explicit warrant** (the ledger), **explicit falsifier**
(conflicts/retractions), and **comparable cost** (queries and proposals in the same unit). These three are
exactly the foundation of the unified framework; what is missing is the other forms of judgment.

## 7. Honest boundary: what counts as research, what counts as reskinning

**Reskinning**: write the four operators as four prompts and let the model choose. No warrant, no
falsifier, no cost — "unified" shows up only in the README.

**Real research** (falsifiable): given the same Γ, two different operators produce checkable warrants, and
the scheduler can use cost to pick the better path; if the scheduler picks wrong, that can be falsified by
experiment. The minimal falsifiable proposition:

> On a task that needs both deduction and abduction, scheduling with "shared cost + explicit falsifier"
> takes fewer steps or fewer experiments than "calling the two operators in a fixed order".

This can be run and confirmed, and it can also be run and refuted.

## 8. The next minimal experiment

Do not build the whole framework. Build only **two operators sharing one kernel**:

1. Deduction: rule base + facts → conclusion (warrant: derivation chain);
2. Abduction: observation + rule base → candidate hypotheses (warrant: each hypothesis can derive the
   observation);
3. Scheduling: hypotheses have different verification costs; use cost to choose which to test first;
4. Failure: verification fails ⇒ falsifier fires ⇒ roll back and record (reusing the existing ledger
   structure).

The domain can simply be code: given "the function outputs a wrong value on some input", abduce three
possible causes (boundary, type, side effect), deduce the testable consequence of each cause, then verify
in order of cost.

Deliverable: an `operators.rs` prototype + a demo, able to measure the step difference between "scheduled"
and "fixed order". This is the first step in turning "general" from a slogan into something measurable.
