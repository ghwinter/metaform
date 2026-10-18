# 09 Cammarata's "Neural Networks Are Already Clean": The three points we can use

Source: Cammarata's Neural Networks Are Already Clean (2026; 943 lines). What I read was the argument skeleton, not
all of the experiments. Line numbers refer to that essay.

## 1. What it argues

**In one sentence**: polysemanticity is mainly not the arbitrary packing caused by "superposition", but
**recursive generalization** — neurons extend outward step by step along a few axes (L127–147).

The two predictions are opposite (L153–155):

| | Basis for grouping | Prediction |
|---|---|---|
| Generalization | shared patterns, easy for downstream to read | local, traceable |
| Superposition | mutually non-interfering, to save dimensions | non-local, arbitrary |

The author bets on generalization: an "iy" neuron grows "diy", then grows "do it yourself" (L135).

Alongside this comes a **falsifiable family of tests** (L387–397): blind maximum, ablation by upstream
clustering, near misses, and "explanations that can lose".

## 2. Three points that are useful to us

### 2.1 Blind maximum = reconstruction test

A second agent sees only the explanation and writes an example that "should maximize activation"; if it
is stronger than the best example in the corpus, the explanation passes (L389). This is **requiring the
explanation to have generative capacity**, not merely to fit the data already seen.

Mapped onto our setting: a proposed predicate should not only "split the collision pairs already seen",
it should also be able to **predict objects it has not seen**. Our current `proposer.py` computes coverage
only (see `RESULTS.md` §3.15), and this is exactly the piece that is missing.
→ Actionable: have each proposal carry 3–5 objects "I think belong to S but were not given", scored by
hit rate.

### 2.2 Explanations that can lose = anti-self-assessment

The original sentence at L397: the best criticism targets a specific conjecture, not a summary metric.
This is two phrasings of the same discipline as our "no same-source self-assessment; the adjudicator must
be outside the proposer".

### 2.3 Recursive generalization = the vocabulary growing inside its closure

The neuron's "iy → diy → do it yourself" is in essence **composition and extension along existing axes**.
This is exactly what our T7 calls "composition cannot escape `Def(V)`", seen at the level of neurons: the
extension can go a long way, but it cannot leave the expressive range of the current language. The
difference is that it discusses the growth of weights, while we discuss the expansion of an explicit
vocabulary.

What the two share is one testable sentence: **for anything that "looks like a new concept", first ask
whether it is an extension along existing axes.** If it is, no new atom is needed; if it is not, then one
is. Our `qde gen` (metarule closure) is precisely making this judgment.

## 3. The parts we cannot use (90%)

- Neurons, residual streams, attention, feature visualization: the object of study is the internal
  structure of weights, not "inferring a concept from examples".
- The two interpretability styles, "enumeration vs exploration" (L473–479): a methodological stance, not
  an executable algorithm.
- The many concrete experiments (InceptionV1, ConvNeXt, GPT-2): no overlap with our domains.

**Conclusion**: this piece offers neither a formula nor an algorithm; what it offers is a **test
protocol** (blind maximum) and one **empirical fact** (polysemanticity comes mostly from local extension
rather than arbitrary packing). The former can be borrowed; the latter can serve as circumstantial
evidence for our "composition vs atom" distinction.

## 4. Next step (actionable)

Add blind-maximum-style scoring to `proposer.py`:

1. In the response, besides the predicate, attach 3–5 objects that "I predict positive but that are not in
   the sample";
2. Score them against ground truth (hit rate / reported separately from coverage);
3. If a candidate has high coverage but a low prediction hit rate → judge it as "merely fitting the
   collision pairs" and do not admit it to the vocabulary.

This would raise proposal quality from "how many pairs it splits" to "can it predict", and it is also
closer to what the paper says, that "a proposal is a first-order term".
