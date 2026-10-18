# 12 Operators Are Not a Checklist: Typing Them by "Which Slice of State They Read and Write"

I was reading a 2135-line source conversation, nine rounds of Q&A. The center of gravity of
v1 (1301 lines, 5 rounds) is **representation**: a sample ≠ an object, and the missing primitive is
"re-representation". v2 adds 4 rounds and shifts the center of gravity to **control**:

| | v1 (§1–5) | v2 (§6–9) |
|---|---|---|
| What is missing | the representation layer (vocabulary/predicates/DSL) | the control layer (operator set + selector + unified interface) |
| Keywords | intension φ_S, attribute exploration, distinguishing questions | operators, meta-reasoning, typed state-transition, JEV |
| Form already given | P(h\|E⁺,E⁻,K) | s=(X,R,H,E,C,B), o:s→P(S), argmax E[ΔQ]−λCost |

So the sentence in §8, "operators sound like different quotients/elements", is right: the source conversation gives 15
mechanisms (§7), 5 operator examples (§8), and one state tuple (§9), but **does not give them types**.
Without types, they can only be a checklist.

## 1. The axis of classification was wrong

The 15 mechanisms are classified by **cognitive name** (deduction, induction, abduction, analogy,
abstraction, anti-unification, hierarchy, symmetry, decomposition, causality, inverse planning, active
learning, strategy selection, meta-reasoning, error correction). Classification by name inevitably looks
like a list of coordinate elements, because they are not at the same layer, do not take the same input, and
do not produce the same thing.

The right axis is: **which blocks of the state it reads, and which blocks of the state it writes.** §9 of
The source conversation already cuts the state into six blocks:

```
s = (X, R, H, E, C, B)
    X elements/attributes   R relational structure   H candidate hypotheses/rules   E evidence/counterexamples   C goals/constraints   B budget
```

Once this axis is used, the 15 mechanisms collapse into **5 classes**, and the warrant and falsifier of each
class have a fixed form:

| Class | Reads | Writes | Members (§7 numbering of the source conversation) | warrant | falsifier |
|---|---|---|---|---|---|
| ① Closure | H,C,X,R | a new judgment J (a conclusion about X) | deduction (1), constraint checking | derivation chain, machine-checkable | a premise is false / a rule is false |
| ② Hypothesis generation | X,R,E | H | induction (2), abduction (3), analogy (4), abstraction (5), anti-unification (6), hierarchy (7), causality (10), intent inference (11) | coverage + simplicity / can derive the observation / structural correspondence / compression | one counterexample / a better explanation not ruled out / the mapping breaks a constraint |
| ③ Representation rewriting | X,R,H,E | X',R' (change the vocabulary or change the structure) | representation reconstruction (§8-5), symmetry/quotient (8), decomposition (9) | after rewriting, the old evidence still holds (fidelity) + the description is shorter | after rewriting, the old evidence cannot be reconstructed |
| ④ Evidence gathering | H,C,B | E | counterexample search (§8-4), active learning (12), experiment design | the evidence distinguishes at least two H (discriminating power) | after gathering, the posterior over H is unchanged (money burned) |
| ⑤ Control | all of s + the cost table | the chosen o and B' | strategy selection (13), meta-reasoning (14), error correction (15) | an estimate of the expected ΔQ / Cost | the realized gain is below the random baseline |

Three points are immediately visible:

1. **Deduction is not of the same kind as the other three.** It does not change H; it only computes the
   consequences of H (①). Induction/abduction/analogy are all ②, differing only in the form of the warrant,
   not in type. The intuition in §8, "they are not the same kind of mathematical object" — correct, but it
   can be put precisely: they are the same type with different warrants.
2. **③ is the most lacking class, and also the easiest to miss.** The center of gravity of v1
   (re-representation) falls in this class; among the 15 items of v2 it has only 8 and 9, and the source conversation itself
   admits that "changing representation is undecidable" (§3 cites Kolmogorov's uncomputability).
3. **⑤ cannot use the same warrant as ①②③④.** Its warrant is a **prediction** (whether this step is worth
   it), and its falsifier is **realized gain below the baseline**. The argmax E[ΔQ]−λCost given in §9 of
   The source conversation is precisely this class, but never says where this E[·] comes from — which is
   exactly where one can run experiments oneself.

## 2. Four operational conditions for being brain-like

"Brain-like" is usually rhetoric in engineering. Strip the rhetoric, and among the differences between the
human brain and current LLMs, only four can be written as code, and all four line up with the requirements
of the source conversation:

| Condition | Correspondent in the brain | Engineering construct | Where the source conversation says it |
|---|---|---|---|
| Competition | multiple hypotheses active at once, inhibiting each other | H must be a set, and its members **irreducible** (not restatements of the same answer) | line 1021, "generate a hundred phrasings but never compare a hundred explanations" |
| Prediction error as a universal currency | predictive processing / dopamine error | every operator must be able to emit "what I predicted", for ④ to gather evidence on | line 716 E_predict, line 1683 error-driven correction |
| Offline recombination | hippocampal replay | the ledger is not an audit log, it is the **scheduler's training data**: replay historical (s,o,gain) | line 1094, "keep what rules once worked" |
| Choice under cost | basal-ganglia option selection | the argmax of ⑤ must use **additive** cost (the same unit across operators) | line 2058, line 792 |

The third is the most valuable: it turns §4 of the source conversation, "continuous verification and memory module", from an
archiving feature into **the only path to self-improvement**. Without replay, the E[ΔQ] of ⑤ can only be
hand-written; with replay, it can be estimated from its own history.

**But "brain-like" itself cannot be the goal.** The human brain's working memory holds about 4 items; holding
thirteen constraints across a hundred rounds relies on **writing things down externally** (Clark & Chalmers
1998), not on the brain; and the brain's most typical failures happen to be "stop at the first plausible
explanation" (Simon 1956) and confirmation bias (Wason 1960) — exactly what needs fixing. So the goal should
be written as "**an auditable scaffold**": keep the three mechanisms of competition, prediction error, and
replay, and drop "brain-like" as a criterion.

## 3. A new criterion for unification

At the end of §8, the source conversation says "allowing function composition formally does not mean any composition is
meaningful. Every operator needs its input and output types, preconditions, computational cost, and failure
modes made explicit". This sentence is the closest the whole text comes to an executable specification, but
it stops at "needs".

Turn it into a hard constraint: **every operator must declare a five-tuple**, otherwise it may not enter the
scheduler:

```
op = (read ⊆ {X,R,H,E,C,B}, write ⊆ {X,R,H,E,C,B}, warrant, cost, falsifier)
```

- `read/write` lets ⑤ compute "will this operator run in vain" (when write ∩ existing = ∅ it is not worth it);
- `warrant` makes the output of ② checkable, rather than "whatever the model says";
- `falsifier` lets a failure roll back to a specific step instead of redoing the whole segment;
- `cost` uses the same unit (query count / experiment count / tokens), otherwise it is neither addable nor
  schedulable.

This also answers, in passing, the ambiguity the agent raised about awareness ("it suffices to be aware
that the structure exists", vs line 625, "matrix computation is required"): **"be aware" = the trigger
condition of ③ is met** (the current representation cannot express some already-confirmed evidence), not a
mysterious act of noticing. The observable signal is that the evidence brought back by ④ has nowhere to sit
in X,R.

## 4. Falsifiable propositions

1. **Scheduling beats fixed order**: on the same task, ⑤ with "shared cost + explicit falsifier" uses fewer
   steps or fewer experiments than "①②③④ in a fixed order". Only if it cannot be refuted is the unification
   more than reskinning.
2. **Representation rewriting must be faithful**: every rewrite in ③ must be able to reconstruct all the old
   evidence in the new representation; failing that means information is lost, not that it is "more
   abstract". This one can be knocked down by constructing a counterexample.
3. **Adding operators alone does not add capability**: without explicitly adding inductive bias, merely
   increasing the number of ② operators will not improve monotonically on an open domain (No Free Lunch +
   Gold's ceiling, which the source conversation itself cites). If monotone improvement is observed, the
   improvement comes from bias rather than operator count, and the framework's "unification" claim must give
   ground.

## 5. Next step (minimal)

Do not build the whole framework. Build only the three classes ②+①+④ sharing one kernel, scheduled by ⑤:

- ② Abduction: observation → 3 irreducible candidate causes (warrant: each can derive the observation);
- ① Deduction: each cause → testable consequence (warrant: derivation chain);
- ④ Evidence gathering: pick the test that discriminates the most candidates (warrant: discriminating power);
- ⑤ Scheduling: order by cost, not by a fixed order.

Metric: the number of tests needed to reach the same judgment. Fixed order vs cost scheduling, by how much.
Deliverable: `operators.rs` + a demo + a curve that can be refuted.

---

(The side-by-side of the six readings is in §6, to be filled in.)
