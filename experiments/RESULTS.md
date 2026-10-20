# Domain A experimental results (synthetic formal context)

2026-10-19 · Code: `domain_a.py`, `domain_prior.py` · fully reproducible (fixed seeds)

> **Note.** Command transcripts quoted below are verbatim run records from an earlier build whose
> CLI printed Chinese; the current build prints English. They are kept unedited on purpose.

## 0 One-sentence conclusion

**Asking "which object" is almost worthless; what is valuable is "when to stop asking about objects and start changing the vocabulary."**
The original proposition C3 (information-gain question selection is significantly better than random) is **false** under a uniform posterior and has been downgraded to a negative result.

## 1 v1 (old setup, retired)

10 objects × 6 attributes, hypothesis class = arbitrary Boolean combinations:

| Strategy | Mean number of questions |
|---|---|
| Information gain | 5.41 |
| Random | 8.74 |

It was written as "1.62×" at the time. The problem: under "arbitrary Boolean combinations" the candidate set is a **product space** decomposed by signature class, so asking one question per undetermined class suffices, and any question that splits the set is worth exactly 1 bit. The gap comes only from the fact that "random repeats questions about classes that are already determined."

## 2 v2: tighten the hypothesis class and add a redundancy-removal baseline

300 instances, 12 objects × 8 attributes; the hidden target is the extension of the 9th attribute, which the initial vocabulary lacks.

**E4 (the empirical version of T2)**: the set of hypotheses consistent within V₀ is empty **300/300 (100%)**.

**E2 hypothesis class = arbitrary Boolean combinations (product space)**

| Strategy | Mean questions | Hits |
|---|---|---|
| gain | 7.78 | 300/300 |
| balance | 7.78 | 300/300 |
| **rand_inf** (random only among informative questions) | **7.78** | 300/300 |
| random | 11.30 | 300/300 |
| first | 11.38 | 300/300 |

**E1 hypothesis class = conjunctions of ≤2 literals**

| Strategy | Mean questions | Hits |
|---|---|---|
| gain | 1.96 | 300/300 |
| balance | 1.81 | 300/300 |
| **rand_inf** | **1.86** | 300/300 |
| random | 6.09 | 300/300 |
| first | 5.77 | 300/300 |

**E3 hit rate with a budget of 3 questions (conjunction class)**: gain/balance 100%, rand_inf 98%, random 25%, first 30%.

### Key finding

`rand_inf` ties `gain` exactly (7.78 vs 7.78; 1.86 vs 1.96). In other words:
**the entire benefit comes from "do not ask uninformative questions", not from "choosing more cleverly".**
Under a uniform posterior, every question that splits the candidate set is worth the same number of bits, so which one you pick does not matter.

## 3 E5: after adding a prior, question selection starts to help a little

Prior p(S) ∝ 2^-|S| (shorter conjunctions are more likely), so the posterior is no longer uniform.

| k | gain_w | balance | rand_inf |
|---|---|---|---|
| 2 | **1.63** | 1.86 | 1.87 |
| 3 | **1.79** | 2.04 | 1.87 |

Only a ~9–13% improvement. **Bayesian active learning is a weak lever in this domain.**

## 3.5 Empirical version of T6: more examples only expose vocabulary insufficiency

400 instances, 14 objects × 8 attributes, a* not in V₀; 20 samples per k (8000 runs per row).

| Number of examples k | P(no consistent hypothesis within V₀) |
|---|---|
| 1 | 2.5% |
| 2 | 2.3% |
| 3 | 4.8% |
| 4 | 29.5% |
| 6 | 73.1% |
| 8 | 93.3% |
| 10 | 98.7% |

Reading: once the examples contain a collision where "a positive and a negative example share the same signature", V₀ fails completely; more examples only make collisions more likely.
**So "give it a few more examples" is not a fix, it is an exposure method.** (k=1..3 are low because, on sparse samples, a conjunction of ≤2 literals can still find a hypothesis that "happens to cover" them — this is itself the capacity effect described by T4.)

## 3.6 Cost experiment: the total is determined by "proposal quality"; the stopping rule is a second-order term

Single-missing-predicate model: object query cost 1, proposal cost r, proposal hit probability q, 300 instances.

| q | r | ask_first | propose_first | Difference |
|---|---|---|---|---|
| 0.125 | 5 | 46.74 | 49.44 | +5.8% |
| 0.125 | 20 | 173.26 | 168.33 | −2.8% |
| 0.125 | 100 | 883.73 | 866.39 | −2.0% |
| 0.250 | 20 | 88.13 | 92.86 | +5.4% |
| 0.500 | 5 | 19.56 | 19.74 | +0.9% |
| 0.500 | 100 | 206.06 | 209.73 | +1.8% |

**Reading**:

1. Total cost spans **20 → 884 (44×)**, almost entirely determined by r/q.
2. The difference between the two stopping rules **always stays within ±6%, and its sign flips** (propose_first is slightly better at low q and high r, ask_first is slightly better at high q) — within noise.
3. Conclusion: **scheduling/stopping rules are second-order terms; the real first-order terms are "proposal hit rate q" and "proposal unit cost r".**

So the accurate statement of C3′ is not "vocabulary-level cost dominates" but: **all engineering effort should go into proposal generation** (raise q, lower r),
not into the controller. The controller needs only one rule: when the candidate set is empty, change the vocabulary; then ask again.

## 3.7 Domain C: real code behavior as the oracle

Function under test: `merge_intervals` (merges overlapping intervals; invalid input raises).
12 test cases; the hidden invariant is **the output is shorter than the input** (a merge happened).
Predicates are executable features of the input; the oracle actually runs the function.

The initial vocabulary has 6 features (`empty / single / has_negative / long / sorted / width_gt3`) and **deliberately omits**
"there exists a pair of overlapping or touching intervals". The seed samples +3 and −2 have identical signatures on the 6 features → `insufficient` (T2 triggers).

The resulting trace (`domain_c.py`):

| Step | Status |
|---|---|
| propose `all_positive` (decoy) | insufficient |
| propose `max_len_2` (decoy) | insufficient |
| propose `small_gap` (decoy) | insufficient |
| propose `overlap` | undetermined |
| 8 membership queries | identified |

The induced extension == the hidden target (6 positives, identical). Cost (proposal unit price 20): 4×20 + 8 = **88**;
if the first proposal were correct, it would be **28** — the 3.1× gap is entirely due to proposal order.

What the three decoys share: they take the same value on the colliding pair (`[(1,3),(2,4)]` vs `[(1,2),(3,4)]`),
so they **cannot** change the signature, and the state stays exactly as it was. This is the engineering meaning of T2: a wrong predicate is not "a little help", it is **zero help**.

## 3.8 Domain B: Lean as the oracle (slow, trustworthy, needs batching)

Objects = the 21 integers 11..31; hidden concept = **prime**.
Oracle = `lean check.lean` (no Mathlib, pure core, 2.5 s) computes the whole table in one run;
the file also uses `decide` to **prove** that "on this interval, prime ⟺ coprime to 30".

| Step | Status |
|---|---|
| seed +11 / −25 (same V₀ signature) | insufficient |
| propose `odd` (true on both 11 and 25) | insufficient |
| propose `coprime30` | undetermined |
| 5 queries | identified, **extension == the prime sequence** |

Cost comparison: one batched call = **2.5 s**; per-query invocation = 5 × 2.5 = **12.6 s**.

Two engineering conclusions:

1. **A slow oracle must be batched**, and batching naturally fits "fetch the whole table at once" — the engine consumes tables, not processes.
2. **Decoys must be filtered by "does it split a colliding pair"**, not by semantic intuition. In the first version, `gt_11` looked harmless,
   but it actually split 11 and 25, so the state first became undetermined and then collided elsewhere and fell back to insufficient.
   This shows that "locally separable ≠ globally sufficient", and the state machine discovers it on its own — no extra detector is needed.

## 3.9 Greedy cover vs random vs optimal

200 instances, each with 3 positives × 3 negatives (9 colliding pairs), 10 random candidate predicates.

| Strategy | Mean number of predicates needed |
|---|---|
| Optimal (brute force) | 1.87 |
| Greedy (pick the most covering at each step) | **2.10** |
| Random order | 4.30 |

- **Greedy needs 12% more than optimal**, consistent with the `H_n` approximation for set cover;
- **Random needs 2.05×** greedy.

Implication: T7 says "the vocabulary is sufficient ⟺ the cover exhausts the cross product"; once that holds, proposal changes from "think of a good predicate" into "solve a set cover",
and the latter has ready-made algorithms and provable approximation ratios. `qde cover` is the command-line version of the same thing (`engine/src/main.rs`).

## 3.10 Noisy oracle: T2 goes from a decision to a statistic

Each oracle answer flips with probability ε; the same object is asked r times and the majority wins. 200 trials, 12 objects, 4 attributes, 8 examples.

| ε | r | A false trigger | B missed detection |
|---|---|---|---|
| 0.05 | 1 | 4.0% | 0.0% |
| 0.05 | 5 | 0.0% | 0.0% |
| 0.10 | 1 | 10.0% | 1.7% |
| 0.10 | 5 | **0.5%** | **0.0%** |
| 0.20 | 1 | 9.0% | 18.6% |
| 0.20 | 5 | 0.5% | 0.0% |
| 0.30 | 1 | 12.0% | 20.5% |
| 0.30 | 5 | 8.0% | 8.2% |

- **A false trigger**: the vocabulary was sufficient, but noise creates "positive and negative together" inside one equivalence class → a false report of insufficient.
- **B missed detection**: the vocabulary really is insufficient, but noise smooths the conflict away → no report when one is due.
- Repeating 5 times with majority vote pushes both below 0.5% when ε ≤ 0.2; at ε=0.3 there is still a floor of ~8%.

Engineering conclusion: **critical decisions must be asked repeatedly**, with the repetition count set by the noise rate; `Option<bool>` should be upgraded to a three-state value carrying vote counts.

## 3.11 Full loop + θ gate: default is 3

Each round asks a batch of objects once; each answer flips with probability ε; θ = the vote gap required for confirmation.
Rules: seeds must also be confirmed to θ; a class is not decided while any member has "voted but not reached θ"; conflicts are rechecked for 2 rounds before the vocabulary is changed.
Proposal pool = [decoy, decoy, correct], so a mean of 3.00 proposals and 2.00 wasted is perfect behavior.

| ε | θ | Success | Rounds | Proposals | Wasted |
|---|---|---|---|---|---|
| 0.0 | 1 | 1.00 | 6.0 | 3.00 | 2.00 |
| 0.0 | 3 | 1.00 | 10.0 | 3.00 | 2.00 |
| 0.1 | 1 | **0.25** | 5.7 | 2.61 | 1.75 |
| 0.1 | 3 | **0.98** | 15.6 | 3.00 | 2.00 |
| 0.1 | 5 | 1.00 | 22.1 | 3.00 | 2.00 |
| 0.2 | 1 | **0.07** | 4.9 | 1.95 | 1.34 |
| 0.2 | 3 | **0.83** | 20.9 | 2.96 | 1.97 |
| 0.2 | 5 | 0.99 | 32.0 | 3.00 | 2.00 |

Reading:

1. **θ=1 is a disaster under noise** (only 25% success at ε=0.1) — it "decides" early and hands over a wrong extension.
2. **Raising θ from 1 to 3 jumps success from 0.25 to 0.98**, at the cost of rounds going from 5.7 to 15.6 (about 2.7×).
3. Raising it further to 5 only pushes 0.98 to 1.00 (0.83→0.99 at ε=0.2) and adds another 1.4× in rounds.

An additional run at ε=0.3 (same instances, same seeds): θ=1 gives **0.02 / 4.3 rounds**, θ=3 gives **0.40 / 30.0 rounds**,
θ=5 gives **0.83 / 49.7 rounds**. At 30% noise, θ=5 only reaches 80% success — that is the floor of majority-vote error,
not something a threshold can fix. The right panel of Figure 2 in the paper uses these four columns.

**The default is θ=3** (the default argument of `qde need`). Rule of thumb: `θ ≈ 2 + 10ε`, i.e. use 3 for ε≤0.1 and 4–5 for ε≈0.2.
This judgment is mine, based on the table above — 0.25 → 0.98 is an order-of-magnitude difference, while 0.98 → 1.00 is just cleanup.

## 3.12 Adaptive θ: rejected

Strategy: start at 3, +1 after a confirmed conflict (cap 6), −1 after 5 consecutive conflict-free rounds (floor 3).
150 instances; everything else as in §3.11.

| ε | Strategy | Success | Rounds | Proposals |
|---|---|---|---|---|
| 0.10 | fixed3 | 0.98 | 15.2 | 3.00 |
| 0.10 | fixed5 | 1.00 | 21.9 | 3.00 |
| 0.10 | adapt | 0.99 | 21.9 | 2.98 |
| 0.20 | fixed3 | 0.80 | 21.7 | 2.90 |
| 0.20 | fixed5 | 1.00 | 31.6 | 3.00 |
| 0.20 | adapt | 0.94 | 28.2 | 2.94 |

**Conclusion: adaptive θ is squeezed between the two fixed values and does not dominate.**

- ε=0.1: spends 44% more rounds (21.9 vs 15.2) for 0.01 more success;
- ε=0.2: saves 11% of rounds, but success is 0.94 < 1.00.

So the original rule stands: **choose θ directly from the estimated ε** (ε≤0.1 → 3, ε≥0.2 → 5), with no online adaptation.
The only advantage of adaptation is "no need to estimate ε", but there is a cheaper way to estimate ε: the minority-vote fraction in the votes is a direct estimate of ε.

## 3.13 Choosing θ from estimated ε: good enough, but only for ε≤0.2

`true` = map the true ε to θ (cheating upper bound); `hat` = use the mean minority-vote fraction; `hat_q` = use the 75th percentile.
The mapping table is the same as §3.11 (≤0.1→3, ≥0.2→5, otherwise 4).

| ε | true success / rounds | hat | hat_q |
|---|---|---|---|
| 0.05 | 1.00 / 12.5 | 0.99 / 13.2 | **1.00 / 13.2** |
| 0.10 | 0.99 / 15.5 | 0.99 / 16.3 | 0.99 / 17.3 |
| 0.20 | 0.97 / 31.4 | 0.96 / **25.9** | **0.98 / 28.5** |
| 0.30 | 0.87 / 48.0 | 0.80 / 45.9 | 0.77 / 51.5 |

Conclusion:

1. **The estimator is good enough for ε≤0.2**: within 1 percentage point of the cheating upper bound, and 15% cheaper in rounds at ε=0.2.
2. **It starts to distort at ε=0.3** (0.87 → 0.80/0.77). θ has already hit the cap of 5, so estimation error turns directly into
   misjudgment; this regime needs stronger aggregation (per-object posteriors) rather than a different quantile.
3. `hat_q` is better at low noise and worse at high noise; **no single quantile dominates at every point**, so the kernel keeps the simple mean.

## 3.14 Per-object Beta posteriors: also not fixed, and slower

Each object's votes (y,n) give the posterior Beta(y+1,n+1); for integer parameters there is a closed form
`P(p>0.5) = Σ_{k≤y} C(n,k)/2^n` (n = y+n+1).
Strong-label criterion: `P ≥ 1-δ` (δ=0.05).

| ε | θ=3 | θ=5 | Posterior δ=0.05 |
|---|---|---|---|
| 0.10 | 0.98 / 15.2 | **1.00 / 21.9** | 0.99 / 23.6 |
| 0.20 | 0.83 / 21.2 | **0.97 / 31.5** | 0.96 / 38.7 |
| 0.30 | 0.45 / 28.9 | **0.85 / 49.1** | 0.81 / 76.8 |

**Conclusion: the posterior gate loses.** It is slower than θ=5 at every level, and worse at ε=0.3 (0.81 vs 0.85).

The cause is not an implementation problem: δ=0.05 requires every object to be questioned to very high confidence, doubling the rounds; and the votes added each round
are themselves noisy, so **the longer the run, the more chances some object has to be flipped by noise**. At ε=0.3 the majority-vote error rate
cannot be pushed down anyway (with 9 votes the majority error rate is still close to 10%), so adding votes only extends the exposure time.

### Summary of this line (two negative results in a row)

- Adaptive θ (§3.12): squeezed by fixed values; removed.
- Per-object posteriors (§3.14 in this section): slower and worse; removed.
- The surviving rule is simple: **choose θ from the estimated ε (3 or 5); valid for ε≤0.2**.

To go further, the direction is not tuning thresholds but (a) asking more informative objects (currently only "one representative per class"),
or (b) modeling the noise (global EM to estimate label errors rather than adding votes per object).

## 3.15 Proposer interface (offline): from request to coverage

`proposer.py` packs "colliding pairs + object descriptions + existing predicates" into a JSON request; the model returns **executable expressions**
for candidate predicates; the script evaluates each one and counts how many colliding pairs it covers. In domain C, under the existing 6 features only **4 pairs** actually collide.

| Candidate | Coverage | Note |
|---|---|---|
| `overlap_or_touch` | **4/4** | overlap or endpoint touch → the only solvable subproblem |
| `width_exceeds_gap` | 2/4 | catches true overlaps only, misses endpoint touching |
| `has_repeated_endpoint` | 2/4 | catches touching only, misses true overlaps |
| `input_has_three_or_more` | **0/4** | correlation, not a definition — zero help |

Coverage is directly the input to the T7 set cover: given these four rows, `cover` can compute which one to adopt.

**Honest boundary**: the responses in this round were filled in by the author (`responses_domain_c.json`), with no real API.
So this validates the **interface and scoring pipeline**, not model proposal quality; switching to a real call only requires replacing `load_responses`.

## 3.16 End to end: proposer → qde → adoption

`proposer.py --pool-out --scenario-out` can now directly generate a `qde` scenario and candidate pool. One full run on domain C:

```
{"status":"INSUFFICIENT","action":"PROPOSE","theta":3,"colliding_pairs":[[3,2]],"pending":[]}
cover: overlap_or_touch 劈开 1/1 对
{"chosen":["overlap_or_touch"],"covered":1,"total":1,"remaining":0}
已添加谓词（词汇 7 个）
{"status":"UNDETERMINED","action":"ASK","theta":3,"classes":10,...}
```

Three things are validated at once by this chain:

1. **Proposal**: of the four candidates from the model, only `overlap_or_touch` covers all colliding pairs;
2. **Adoption**: `cover` selects it by the T7 coverage count, and after adding it to the vocabulary the number of equivalence classes goes from 8 to 10 (it really splits them);
3. **State progression**: `INSUFFICIENT` → `UNDETERMINED`, and the action changes from `PROPOSE` to `ASK`.

One interaction detail is worth recording: seeds must be given θ votes. With only one `pos/neg` vote, θ=3 treats them as weak votes,
the state stays at `UNDETERMINED`, and `cover` never triggers — **this is how the noise gate and the proposer mesh**.

## 3.17 Empirical proposal quality q: cost ≈ r/q + rounds

Run in the **real loop** (not an abstract model): each proposal costs $20 and is correct with probability q; otherwise it proposes a decoy built from a random combination of existing
predicates (necessarily useless by T2); once the correct predicate is in place, ask round by round. 200 instances.

| q | Total cost | Theoretical r/q | Proposals | Wasted | Rounds | Success |
|---|---|---|---|---|---|---|
| 1/8 | 162.7 | 160 | 7.75 | 6.75 | 7.56 | 1.00 |
| 1/4 | 88.9 | 80 | 4.07 | 3.07 | 7.56 | 1.00 |
| 1/2 | 49.4 | 40 | 2.09 | 1.09 | 7.56 | 1.00 |
| 1 | 27.6 | 20 | 1.00 | 0.00 | 7.56 | 1.00 |

Three things:

1. **Total cost ≈ r/q + rounds**: 20/q + 7.56 is close to the measurements (27.6, 49.4, 88.9, 162.7); the residual comes from sampling in the
   proposal count — the proposal counts are 1.00, 2.09, 4.07, 7.75, so for q<1 it does not land exactly on 20/q.
2. **Rounds are independent of q** (always 7.56) — the questioning phase does not know and should not care how good the proposer is.
3. **Wasted proposals ≈ 1/q − 1**: at q=1/8, an average of 6.75 are thrown away.

These numbers upgrade the cost decomposition of §3.6 from a "model assumption" to something that "holds in an executable loop".

## 3.18 Multi-seed variance (5 seeds × 150 instances)

| Questioning strategy | Mean ± sd | Per seed |
|---|---|---|
| gain | **2.01 ± 0.06** | 1.95 / 2.08 / 2.07 / 1.96 / 2.00 |
| rand_inf | **1.91 ± 0.07** | 1.83 / 1.97 / 1.99 / 1.85 / 1.93 |
| random | 6.14 ± 0.23 | 6.25 / 6.41 / 6.21 / 5.83 / 5.98 |

| Cover strategy | Mean ± sd | Per seed |
|---|---|---|
| greedy | **2.10 ± 0.06** | 2.00 / 2.12 / 2.09 / 2.11 / 2.15 |
| random | 4.28 ± 0.09 | 4.26 / 4.14 / 4.33 / 4.29 / 4.38 |
| optimal | 1.85 ± 0.06 | 1.76 / 1.87 / 1.86 / 1.85 / 1.91 |

Both conclusions hold, and are stronger:

1. **gain not only has no advantage, it is slightly worse than rand_inf** (2.01 vs 1.91, a gap of 0.10 ≈ 1.5 sd). So the paper cannot say
   "information gain is better", only "no advantage"; the negative result now has variance behind it.
2. **The gap between greedy and optimal (0.25) is 4 sd**, and the gap between random and greedy (2.18) is larger — the coverage claim is solid.

## 3.19 Proposal scoring upgrade: coverage + precision/recall + prediction hit rate

Borrowed from Cammarata's "blind maximization" protocol: looking only at "how many pairs it splits" cannot distinguish
"**sound but narrow**" from "wrong". We add P/R against the hidden target, plus the hit rate of the candidate's own "I predict positive" claims.

Domain C (4 colliding pairs; target = output shorter than input):

| Candidate | Coverage | P | R | Prediction hits |
|---|---|---|---|---|
| `overlap_or_touch` | 4/4 | **1.00** | **1.00** | 3/3 |
| `width_exceeds_gap` | 2/4 | 1.00 | 0.67 | 3/3 |
| `has_repeated_endpoint` | 2/4 | 1.00 | 0.33 | 2/2 |
| `input_has_three_or_more` | 0/4 | 0.67 | 0.33 | 2/3 |

Reading:

1. **Same coverage, different meaning**. `width_exceeds_gap` and `has_repeated_endpoint` both split 2/4, but the former
   has recall 0.67 and the latter 0.33 — the former is closer to the target. Looking only at coverage would misjudge.
2. **P=1.00 does not mean correct**. The last two are "sub-concepts of the target": everything they say is right, but incomplete. They are useful as
   intermediate steps in a set cover, but cannot be submitted as S.
3. Only the unique candidate with P=R=1.00 should enter the vocabulary. This turns "proposal quality is a first-order term" from a slogan into an operational criterion.

**Already in effect in the pipeline**: `proposer.py --pool-out` by default writes only P=R=1 candidates.

```
[pool] kept 1/4 (P=R=1) → pool.txt
[pool] filtered: has_repeated_endpoint, input_has_three_or_more, width_exceeds_gap
```

`qde cover` therefore only picks from qualified candidates. `--pool-all` disables the filter and reproduces the table above.

**But P/R cannot be computed at deployment time** — it uses the hidden extension. The deployment equivalent: turn the candidate's self-reported `predictions`
into queries for the oracle, compute **empirical** precision from the real answers, and eliminate candidates whose prediction hit rate is low. This is exactly the executable version of "blind maximization" in a real system:
an explanation must generate testable predictions, not merely explain the data already seen.

## 3.20 Filtering on the deployment path: can guarantee soundness, not completeness

Same candidates, same pool, two different filtering rules:

| Rule | Basis | Kept |
|---|---|---|
| `--pool-filter offline` | hidden-extension P/R (P=R=1) | **1/4**: only `overlap_or_touch` |
| `--pool-filter empirical` | ask the oracle about the candidate's predictions; keep only if all are confirmed | **3/4**: only `input_has_three_or_more` is eliminated |

Confirmed case by case:

```
overlap_or_touch         确认 3/3
has_repeated_endpoint    确认 2/2
width_exceeds_gap        确认 3/3
input_has_three_or_more  确认 2/3   ← 唯一被淘汰
```

**Conclusion: empirical filtering enforces soundness only, not completeness.**

- The predictions of `has_repeated_endpoint` and `width_exceeds_gap` are all correct — they say nothing false, just incomplete things (P=1.00, R=0.33/0.67).
- At deployment time there is no hidden extension, so recall cannot be measured directly; the only measurable thing is "whether the predictions are confirmed by the oracle".

So the deployment-path rule becomes two rules:

1. **sound**: all predictions are confirmed (measurable);
2. **enough to cover colliding pairs**: `covers > 0`, and the greedy `cover` decides how many more candidates are needed (measurable).

As for "is it complete enough", the only recourse is to ask more, or to have a second agent construct counterexamples — this is the reverse use of blind maximization:
have another agent find objects that "the candidate says are negative but are actually positive". This was not implemented in this round.

## 3.21 Reverse blind maximization: a deployable completeness probe

Idea: have the other side propose objects that "**you say are negative, but I think are positive**". Once the oracle confirms one, the candidate is proven incomplete.
This uses only oracle answers and needs no hidden extension, **so it works at deployment time**.

Domain C:

| Candidate | Probe | Confirmed | Verdict |
|---|---|---|---|
| `overlap_or_touch` | case 2 (disjoint) | 0 | **SURVIVED** |
| `has_repeated_endpoint` | case 4 (true overlap, no repeated endpoint) | 1 | **INCOMPLETE** |
| `width_exceeds_gap` | case 6 (endpoints touch) | 1 | **INCOMPLETE** |
| `input_has_three_or_more` | case 3 (two intervals but they merged) | 1 | **INCOMPLETE** |

Each of the three "right but incomplete" candidates is broken by a single shot; the correct one is not broken (the probe is rejected by the oracle).

**Cost**: one oracle call per candidate. Benefit: it closes the gap from the previous section, "only soundness can be guaranteed" —
soundness now comes from prediction confirmation and completeness from a reverse probe, and both need only the oracle.

**Limitation**: probe quality depends on the side proposing counterexamples. If it cannot think of one, the candidate passes with the label "not broken" —
this is the UNPROBED/SURVIVED distinction, and reports must keep them separate; "not broken" does not mean "correct".

## 3.22 Probe ledger: keep a record of counterexamples that broke something

The first round's probe results are written to `probe_ledger.json`; in the next round every candidate must first pass the ledger —
**a candidate that excludes any confirmed positive is rejected outright, with no need to probe again**.

| Round | Ledger | Result |
|---|---|---|
| 1 (empty) | — | three candidates are broken by probes; the ledger records `[3, 4, 6]` |
| 2 (populated) | 3 counterexamples | the same three candidates additionally get `INCOMPLETE(ledger)` |

The key row is **`repeat_mistake_v2`** in round 2: it is a newly proposed candidate with no probes (round 1 would call it
`UNPROBED`), but because it excludes the ledger's counterexamples it is killed immediately in round 2, with 2 violations.

```
repeat_mistake_v2      probes=0  confirmed=0  ledger=2  INCOMPLETE(ledger)
```

**This is the point of the ledger**: you do not have to wait for someone to think of another counterexample; repeated mistakes are blocked by the historical record.

## 3.23 Two roles: definition vs cover element

If the ledger rejected everything outright, it would also kill "narrow but sound" candidates — which the T7 set cover needs. Split into two criteria:

| Criterion | Requirements | Use |
|---|---|---|
| `definition` | no ledger violation + no violation of confirmed positives + all predictions confirmed + P=R=1 | submit as the complete definition of $S$ |
| `cover` | all predictions confirmed (sound) + splits at least one pair | as an element of the set cover (may be narrow) |

Round 2 with the ledger:

| Candidate | definition | cover | Note |
|---|---|---|---|
| `overlap_or_touch` | **Y** | **Y** | the only one usable as a definition |
| `has_repeated_endpoint` | n | **Y** | narrow but sound; can be used in the cover |
| `width_exceeds_gap` | n | **Y** | same as above |
| `input_has_three_or_more` | n | n | its prediction is rejected by the oracle, so it is not even a valid cover element |
| `repeat_mistake_v2` | n | n | gave no predictions — it cannot pass just because it is "not yet falsified" |

Two rules:

1. **sound must be backed by evidence**: `sound = predictions were given and all were confirmed`. An empty prediction does not count as sound, otherwise any candidate could sneak through.
2. **Narrow does not mean wrong**: narrow candidates can enter the cover pool but cannot serve as the definition. This is exactly T7's "combine several predicates".

## 3.24 The ledger is an editable knowledge base, not an append-only set

`ledger.py` upgrades the ledger into records with status and history:

| Operation | Semantics |
|---|---|
| `add` | add a counterexample (records by / round) |
| `retract ID --reason` | retract: `status=retracted`; excluded from decisions, but the record is kept |
| `amend ID --case N --reason` | amend: correct the object or type; before→after is written to history |
| `list [--all]` / `history` | view the currently valid set and each record's change history |

**Retraction must actually take effect**. Round 3 measurement (retracting the case 6 entry):

```
width_exceeds_gap   probes=1 conf=1 ledger=0  INCOMPLETE   ← 之前是 ledger=1, INCOMPLETE(ledger)
```

The ledger violation count drops from 1 to 0, and the verdict falls back to the probe's `INCOMPLETE` — retraction is not deleting history, it is removing it from the decision.

**Amendments also leave a trace**:

```
L1 case 5 [active]
    add      found by has_repeated_endpoint
    amend    (4, 'positive') -> (5, 'positive'); 当时记错了对象
L3 case 6 [retracted]
    add      found by width_exceeds_gap
    retract  oracle 回答待复核，先不计入
```

Why editability is required: a counterexample may come from a noisy oracle, may record the wrong object, or may hold only for a certain class of candidates.
An append-only set would freeze one mistake forever, and every later candidate would be killed by it — **the same principle as T6:
"more data" does not automatically become "better data" unless retraction is allowed.**

## 3.25 Conditional expiry: suspend on target change instead of deleting

A counterexample is confirmed "under a particular target concept". When the target changes (the user changes what they want to learn), the old counterexample may no longer hold —
but it cannot simply be deleted either, because deleting loses the evidence.

`ledger.py retarget --to S1` turns every active entry into `stale`: **excluded from decisions, but kept**;
each entry becomes effective again only after `revalidate`.

| Status | Participates in decisions | Meaning |
|---|---|---|
| `active` | yes | valid under the current target |
| `stale` | no | the target changed; waiting for reconfirmation |
| `retracted` | no | retracted (reason on file) |

Measurement (5 candidates):

```
全 stale:        所有 ledger=0，判定退回探针给出的 INCOMPLETE
revalidate L2:   input_has_three_or_more ledger=1，width_exceeds_gap 仍为 0
```

`width_exceeds_gap` excludes case 6 (which has been retracted), so it is unaffected by L2 — **per-entry granularity is right,
not one-size-fits-all**.

The history chain is complete:

```
L2 case 3 [active]
    add         found by input_has_three_or_more
    stale       retarget S0 -> S1: 用户把目标从"输出变短"改成…
    revalidate  在目标 S1 下重新确认；新目标下 case 3 仍然为正
```

## 3.26 Every verdict must answer "on what grounds"

Entries gain an `evidence` field:

```json
"evidence": {
  "source": "domain_c: merge_intervals(output) < input",
  "answer": true,
  "votes": "3 0",
  "recorded": "2026-10-19 04:05:12"
}
```

- `source`: where the judgment came from (test, Lean, oracle call, human);
- `answer`: what the oracle said at the time;
- `votes`: for a noisy oracle, the vote counts at the time;
- `recorded`: when it was recorded.

Old entries are backfilled with `normalize` (`source` is set to the `by` of the time, with nothing invented): `normalized: 3 entries now carry evidence`.

Changing the source also goes through `amend --by`: this round corrected a record that attributed case 7 to `width_exceeds_gap`
(the one that actually excludes 7 is `input_has_three_or_more`).

**Verification**: after adding case 7 to the ledger, each candidate's violation count exactly matches its extension —
`width_exceeds_gap` (contains 3, 7) has 0 violations, `has_repeated_endpoint` (contains only 6, 11) has 2. The ledger and the predicate extensions agree.

## 3.27 Evidence reliability: hard and soft tiers

`evidence.reliability ∈ [0,1]`: unit tests / Lean are recorded as 1.0, a model oracle as $1-\varepsilon$. Verdicts come in two tiers:

| Tier | Condition | Effect |
|---|---|---|
| Hard violation | reliability ≥ 0.8 | directly `INCOMPLETE(ledger)` |
| Soft violation | < 0.8 | only marks it (the `soft` column), does not block |

Measurement (`width_exceeds_gap` against case 11):

```
L5 可信度 0.60 → hard=0 soft=1  → verdict 仍是 INCOMPLETE（来自它自己的探针）
amend 到 1.00  → hard=1 soft=0  → verdict 变 INCOMPLETE(ledger)
```

**Significance**: model statements cannot carry the same weight as unit tests. Low-reliability entries are still recorded and still displayed, but have no veto —
they are neither ignored nor allowed a single-vote veto. This is continuous with T6/§3.24: **evidence must be correctable and also gradable.**

## 3.28 Cross-confirmation: merge weak evidence by noisy-OR

The same object can have several independent pieces of evidence. Merge rule: `1 - Π(1 - r_i)` (independence makes it noisy-OR).

Measurement (case 4, three model evidence items with r=0.5):

| Count | Merged reliability | Verdict |
|---|---|---|
| 2 | 0.75 | soft violation (`soft=1`), does not block |
| 3 | **0.88** | hard violation (`hard=3`) |

`corroborate --apply` merges them into one new entry:

```
L6/L7/L8 case 4 [superseded]   合并进 L9（r=0.88）
L9       case 4 [active]       corroborate L6, L7, L8 → r=0.88
```

Key points:

1. **A structured version of repeated questioning**. A single weak piece of evidence has no veto, but three independent sources crossing do — this is
   the ledger form of "repeated voting" from `RESULTS.md` §3.11.
2. **Not simple addition**. Three r=0.5 items would sum to 1.5 (meaningless); noisy-OR gives 0.875, bounded by 1.
3. **The original entries are kept as superseded**, with a pointer to the merge — the evidence chain is not broken.

## 3.29 Direction conflict: noisy-OR must not treat "contradictory" as "mutually supporting"

noisy-OR looks only at reliability, not direction. If one says positive and one says negative, merging by the formula gives 0.99 — counting **disagreement** as **consensus**.

New rule: group by direction first; if both sides reach 0.5, declare a **conflict** and leave the object's truth undetermined:

| Status | Participates in decisions | Handling |
|---|---|---|
| Conflict | no | stop blocking any candidate; enter the re-ask queue |

Measurement:

```
case 4: pos=0.88  neg=0.90   → CONFLICT
hard 计数：3 → 2（case 4 不再拦人）
retract 错误的那条（否定来自坏来源）→ no conflicts → hard 回到 3
```

Key points:

1. **Conflict is not averaging**. Neither 0.88 nor 0.90 can override the other; the right move is to mark the truth as unknown.
2. **Conflict must block the verdict, not the process**. The system keeps working, but stops drawing conclusions about that object.
3. The only resolution is to ask again — then retract the wrong entry, keeping the reason in history.

## 3.30 Conflicts become a re-ask queue, with an "overwhelming" criterion

A conflict cannot be just "exclude + print"; it must become an executable next step.

### Queue

`ledger.py conflicts` outputs in **ascending gap order**: the smaller the gap, the earlier the question, and it gives "how many more independent confirmations are needed"
(set so that one side reaches 0.95, solved backward from `r_query`):

```
 case    pos    neg    gap   再问
    4   0.88   0.90   0.03     4
    3   1.00   0.95   0.04     4
```

`--json` outputs the same content for a higher-level scheduler.

### Overwhelming criterion

The first rule was "both sides ≥0.5 means conflict", but `pos=1.00 vs neg=0.55` was also called a conflict — **overwhelming evidence
was canceled by a weak opposing vote**. A ratio constraint was added: a true conflict requires `min ≥ ratio × max` (default 0.7).
So 1.00 vs 0.55 is no longer a conflict (the stronger side speaks), while 0.88 vs 0.90 and 1.00 vs 0.955 still are.

### Effect on verdicts

Conflicted objects do not count toward hard/soft violations, so `has_repeated_endpoint` drops from hard 3 to 1 (only case 7 remains),
and `width_exceeds_gap` drops to 1. **During a conflict the system draws no conclusion, but it does not stall.**

## 3.31 Re-asking also needs coverage: merge conflicts in the same class into one query

If conflicted objects share an equivalence class, asking one member confirms the whole class — **T7's coverage argument applies to the conflict set**.

`proposer.py --plan` produces a plan:

```
冲突对象 [3, 4, 6]；按当前词汇合并成 2 次询问：
 class      conflicted  ask        values
     2          [3, 6]    3   3:1.00/0.96 6:0.90/0.90
     3             [4]    4        4:0.88/0.90

最少 2 次询问（每类一次），对比逐个问的 3 次
```

Key points:

1. **Queries drop from 3 to 2**: the members of class 2 are `[2, 3, 6]`, and 3 and 6 are in the same class.
2. This is the same logic as the §3.10 set cover, with the covered objects changed from "colliding pairs" to "conflicted objects".
3. The cost is **asking one representative**: if the class is impure under the conflict, more queries are needed — so this is a lower bound, not a guarantee.

## 3.32 Purity check in the plan: do not ask one representative when the class is impure

The previous section assumed "pure within a class". With the purity check added:

| Purity | Basis | Queries |
|---|---|---|
| `impure(known)` | the class contains both confirmed positives and negatives | ask every conflicted member once |
| `pure?(single)` | the class has one member | 1 |
| `unknown` | the class has several members but no confirmed labels on both sides | ask the representative once (risky) |

Measurement:

```
 class  size  known +/-   purity        conflicted  asks
     2     3        0/1   unknown          [3, 6]     1
     3     1        0/0   pure?(single)       [4]     1
     7     3        1/1   impure(known)      [10]     1
合计 3 次询问；逐个问需 4 次
```

Key points:

1. **Conflicted objects do not count as known labels** — otherwise "conflict" would be treated as "known", which is self-contradictory. This came out of fixing a bug.
2. In this domain equivalence classes have at most 3 members, so `impure(known)` rarely changes the total; for larger classes it forces one query per member.
3. `unknown` is an explicit risk: ask only the representative; if a dispute arises again after the answer, more members must be asked.

## 3.33 Query budget for unknown classes: gamble or buy insurance

For unknown purity, do not hard-guess 1; choose by expected cost:

- Ask 1 representative: `1 + (1-p)·(c-1)·R` (if the class is impure, the remaining `c-1` members must be relabeled, each costing `R`)
- Ask all conflicted members: `c`

where `p` is the prior purity (default 0.5) and `R` is the rework cost of one mislabel (relative to one query).

| R | cost(1) | cost(all) | Choice | Total queries |
|---|---|---|---|---|
| 1 | 1.50 | 2.00 | ask 1 | 3 |
| 3 | 2.50 | 2.00 | **buy insurance** (`unknown(buy-safe)`) | 4 |

Key points:

1. **The decision flips with cost**: gamble when mislabeling is cheap, ask one by one when it is expensive. This is not a guess; it is a comparison of `R`.
2. Isomorphic to the §3.10 cost model: there it compares "ask first or propose first"; here it compares "ask 1 or ask c".
3. For `c=1` there is no bet; just ask.

## 3.34 Purity prior p is no longer hand-filled: estimated from the ledger

`p` (the probability that a multi-member class is pure) was previously fixed at 0.5. It now defaults to an estimate:

- count only classes with **at least two confirmed labels** (only those carry information);
- Laplace smoothing: `p̂ = (impure + 1) / (total + 2)`;
- fall back to 0.5 when there is no such data, and say so in the output.

Measurement:

```
[plan] p(pure)=0.67  estimated from ledger (1/1 impure)
 class 2 ... cost(1)=2.00 cost(all)=2.00 → 问 1 个（决策从 buy-safe 翻回）
```

Key points and caveats:

1. **Decision flip**: with p=0.5 and R=3 it chose buy-safe (2.50 > 2.00); after estimating 0.67, cost(1)=2.00, so it asks 1.
2. **The sample has only 1 class, so the estimate is fragile**. 2.00 vs 2.00 is a knife-edge; a small difference flips it. Printing `1/1 impure` in the output is meant to make that visible.
3. As labels accumulate, the estimate stabilizes — **parameters come from data, not from defaults**.

## 3.35 The last hand-filled parameter, R, also comes from the ledger

`R` is "the rework cost of one mislabel (unit: one query)". The current estimate:

- numerator: the number of repair actions that actually occurred in the ledger (`retract` + `amend` + `supersede`);
- denominator: the number of entries that needed repair (have a repair action, or are already retracted/superseded);
- `R = max(1, numerator/denominator)` (floor 1: one repair costs at least as much as one query).

Current ledger:

```
[plan] R=1.22  estimated from ledger (11 repairs / 9 repaired entries)
[plan] p(pure)=0.67  estimated from ledger (1/1 impure)
```

So the comparison for unknown-purity classes becomes `cost(1)=1.41` vs `cost(all)=2.00` → ask 1.
Forcing `--rework-cost 3` switches it to buy-safe. **The decision follows the data, not the defaults.**

At this point all three quantities in the cost comparison have a source:

| Quantity | Source |
|---|---|
| `c` (number of conflicted members) | structure (equivalence class) |
| `p` (class purity) | ledger, Laplace (§3.34) |
| `R` (rework cost) | ledger, repairs/entries ratio (this section) |

Residual risk: the denominator has only 9 entries, so the estimate is unstable; therefore the output always prints the raw ratio, such as `11/9`, so a human can judge credibility.

## 3.36 Ledger semantics move into Rust: direction, conflict, confirmed

The Rust ledger in `metaform` now carries the three rules from the Python version:

1. **Direction merge**: several independent evidence items for the same object merge into `pos` / `neg` by noisy-OR;
2. **Conflict test**: a conflict requires both sides ≥0.5 and `min ≥ 0.7·max` (overwhelming evidence is not canceled by a weak opposing vote);
3. **Confirmed**: `pos ≥ 0.5` and not in the conflict set.

On this repository's own ledger (21 migrated records):

```
$ metaform ledger conflicts --project .
{"conflicts":[3,4,6,10]}

$ metaform ledger cases --project .
{"cases":[7,11]}
```

Consistent with the Python version (`ledger.py conflicts` / `proposer.py --plan`): conflicted objects are excluded, leaving only 7 and 11.

Rust tests increased from 12 to **15** (3 new: noisy-OR merge, conflict requires comparable sides, confirmed excludes conflicts),
and the expected count in `check-rust.ps1` was updated to 15.

## 3.37 One-shot invocation: `state.json`

An agent should not have to write a script just to ask one question. `metaform state` stores the scenario in `.metaform/state.json`:

```powershell
metaform state init --project . --n 4
metaform state add  --project . --name p --ids 0,1
metaform state vote --project . --id 0 --yes 3 --no 0
metaform state vote --project . --id 1 --yes 0 --no 3
metaform state need --project .
```

The output has exactly the same shape as script-mode `need`:

```json
{"status":"INSUFFICIENT","action":"PROPOSE","theta":3,"epsilon_hat":0.00,
 "theta_rec":3,"classes":2,"free_classes":1,"colliding_pairs":[[0,1]],
 "pending":[],"suggested_labels":[2]}
```

`state.json` is human-readable:

```json
{"n":4,"predicates":[{"name":"p","ids":[0,1]}],
 "votes":[{"id":0,"yes":3,"no":0},{"id":1,"yes":0,"no":3}]}
```

Rust tests 15 → **17** (new: state save/load round trip, rebuilding the state machine from state).

## 3.38 MCP: from "can be called" to "can be mounted"

`metaform mcp` starts a stdio JSON-RPC server exposing four tools:

| Tool | Purpose |
|---|---|
| `state_need` | read `state.json` and return the state-machine JSON (same as the CLI's `need`) |
| `state_vote` | record a vote and write it back |
| `ledger_cases` | confirmed objects with no conflicts |
| `ledger_conflicts` | objects with direction conflicts and undetermined truth |

Measurement (feeding request lines to the process):

```
{"id":3,...} → {"content":[{"text":"{\"status\":\"INSUFFICIENT\",\"action\":\"PROPOSE\",...,\"colliding_pairs\":[[0,1]]}"}]}
{"id":4,...} → {"content":[{"text":"{\"cases\":[7,11]}"}]}
{"id":5,...} → {"content":[{"text":"{\"conflicts\":[3,4,6,10]}"}]}
```

Key points:

1. **One kernel, two front ends**: the CLI for humans, MCP for agents to mount; the `need` JSON is identical.
2. Notifications (`notifications/initialized`) get no reply — the protocol requires it.
3. An unknown tool returns content with `isError: true` instead of crashing the connection.

Rust tests 17 → **21** (4 new: initialize, tools/list, unknown tool, notification).

## 3.39 `report` and the first release (v0.2.0)

`metaform report` writes the state to `out/report.md` (human-readable), drawing on `state.json` + `ledger.tsv`:

```markdown
## 状态
- **INSUFFICIENT** → 动作 `PROPOSE`（θ=3）
- 等价类 2 个，未决类 1
- 撞车对 1 个：(6,2)

## 覆盖
- 贪心选择 overlap_or_touch：覆盖 1/1 对，剩余 0

## 证据台账
- 条目 21：active 14 / stale 1 / superseded 3 / retracted 3
- 冲突（真值未定）：3, 4, 6, 10
- 纯度 p=0.67（1/1 impure, Laplace），返工代价 R=1.21（1 + 3/14 retracted）
```

Release: `scripts/release.ps1` → `dist/metaform-0.2.0-win64.zip` (4 entries, 276.2 KB);
unzip it and you get `metaform.exe` + `SKILL.md` + `README.md` + `VERSION.txt`. Measured after unzipping:

```
$ metaform.exe ledger cases --project .
{"cases":[7,11]}
```

Rust tests 24 → **26** (two report cases: INSUFFICIENT names the collision, IDENTIFIED recommends stopping).

## 3.40 `score` and v0.3.0

Proposal scoring enters Rust: the input is an **extension** (object ids) rather than executable code, so it can run without Python.

Real domain C data (4 candidates, ground truth 6 positive and 6 negative):

| Candidate | Coverage | P | R | Predictions | Ledger violations | Verdict |
|---|---|---|---|---|---|---|
| `overlap_or_touch` | 1/1 | 1.00 | 1.00 | 3/3 | 0 | **definition** |
| `has_repeated_endpoint` | 1/1 | 1.00 | 0.33 | 2/2 | 1 | cover only |
| `width_exceeds_gap` | 0/1 | 1.00 | 0.67 | 3/3 | 1 | — |
| `input_has_three_or_more` | 0/1 | 0.67 | 0.33 | 2/3 | 1 | — |

Reading: only `overlap_or_touch` can serve as the definition (P=R=1 and excludes no confirmed positive);
`has_repeated_endpoint` is right but too narrow, so it can only be a cover element — consistent with §3.23.

Release: `metaform-0.3.0-win64.zip` (4 entries, 290.3 KB); after unzipping, both `score` and `report` work.

Rust tests 26 → **29** (3 new: perfect candidate, narrow candidate, ledger violation).

## 4 What this means for the paper

1. **C3 is retired**, replaced by a negative result: *object-level question selection barely matters under uniform cost*.
2. **The valuable decisions are at the vocabulary level**: when to stop asking about objects and switch to proposing predicates; which one to propose; how much oracle budget to spend.
   Only this layer has non-trivial cost, because the "unit cost" of a predicate proposal is far higher than a membership query (a human must think, an LLM must generate, and it must be verified).
3. The paper's experiment matrix must change: the primary metric goes from "number of questions" to **"recovery rate under total cost (object queries + predicate proposals)"**,
   comparing three stopping rules: switch only when F is empty, switch when the budget runs out, and switch by expected gain.
4. This is consistent with T2: since search within the vocabulary is bound to fail, optimizing object-level selection is optimizing a process that is doomed to fail.

## 5 Reproduction

```powershell
$py = $env:METAFORM_PYTHON
cd experiments
& $py domain_a.py --n 300 --objects 12 --attrs 8 --k 2   # E1–E4, about 4 s
& $py domain_prior.py                                     # E5, about 6 s
```

## 6 Next experiments

1. **Cost-sensitive stopping rules**: with unit costs c_obj ≪ c_pred, compare the total-cost/recovery-rate curves of the three stopping rules.
2. **Predicate proposal efficiency**: random proposals vs difference-driven proposals (using attribute differences at the collision between positive and negative examples), to see how many proposals are needed before F becomes non-empty.
3. Repeat 1 and 2 on domain B (Lean-decided) and domain C (test-decided) to confirm the conclusions are not artifacts of the synthetic domain.



## 7 Three proposers, one validator (v0.5.4)

A family gives every arm the same state -- objects, attributes, the vocabulary in force, and a few
labelled examples -- and one proposal each. All proposals go through `metaform propose`, and the
driver counts how many separate every pair that the *true* labels make indistinguishable under the
vocabulary in force. Three families, all deterministic (`proposer_arms.py --family <f>`):

| family | target | instances | random | vsa | llm (this session's model) |
|---|---|---|---|---|---|
| k2 | conjunction of 2 attributes | 10 | 0.200 | 0.100 | **1.000** |
| k3 | conjunction of 3 attributes | 5 | 0.000 | 0.000 | **0.800** |
| noisy | k2 with one wrong evidence label | 5 | 0.200 | 0.000 | **1.000** |

At `r = 20` the cost column is `r/q + 7.6`: 27.6 for q = 1.0, 32.6 for q = 0.8, and infinite where q = 0.

What the families show:

1. **The model arm carries the loop.** It solves k2 and noisy outright and misses one k3 instance.
   The miss is informative: with only four labels, several three-attribute conjunctions agree with
   the evidence, and the rule used here (smallest consistent extension) chose one that leaves a true
   positive out. The failure is a property of the evidence, not of the protocol.
2. **Retrieval is at or below chance in every family.** With one predicate in the vocabulary and
   four labelled objects, a contrast vector is too thin a signal; adding 3-conjunctions to its
   candidate pool (arity 3) did not help. The earlier 0.810 measurement used twelve objects and a
   full attribute pool, so the component is not useless in general -- it is useless *here*, which is
   exactly the boundary the protocol is meant to expose.
3. **The noisy family is about distrust, not accuracy.** One labelled negative is truly positive; the
   model arm recovers the true concept anyway, while both mechanical arms keep fitting the wrong
   label. `metaform propose` accepts all three arms' candidates, because being well-formed and being
   true are different questions.

`scripts/check-components.ps1` re-runs all three families and fails if the model arm drops below
0.9 / 0.8 / 0.9, or stops beating retrieval.
### 7.1 The same loop on real files from this repository

`experiments/real_domain.py` replaces the synthetic objects with the repository's own files: thirty
Python experiments and scenario files, nine attributes read off their contents, and ten hand labels
for one real distinction -- which files drive the tool by running a process. Five are labelled
positive, five negative, and the labels match the mechanical truth with no disagreement.

The vocabulary in force is `a_json, a_assert, a_small`, three attributes that say nothing about
driving a process. On this real state the kernel reports `INSUFFICIENT` with six colliding pairs:
`llm_e2e.py`, `propose_loop.py` and `proposer_arms.py` are indistinguishable from `ledger.py` and
`llm_mock.py`. The two arms then propose:

| arm | proposal | separates |
|---|---|---|
| retrieve | `a_ledger` | 3 of 6 colliding pairs |
| the session's model | `runs_a_process` (the files that mention subprocess, cargo or the kernel binary) | 6 of 6 |

The retrieval arm is not absurd here -- `a_ledger` is a real attribute and it does split half the
pairs -- but it is not the concept, and nothing in the vector signal says so. The kernel does: it
reports how many pairs a proposal separates, separately from whether the proposal is well formed.

This experiment is deliberately not part of `check-components.ps1`: its objects are the repository,
so a file added tomorrow changes the state. It is run by hand and recorded here.

### 7.2 The ILP component

`crates/ilp` is a small top-down relational learner: a clause is a head over fresh variables and a
body grown from a fixed signature; a candidate is kept only if it covers every positive example and
no negative one, and only if every head variable occurs in the body (without that rule a clause such
as `grandparent(V0,V1) :- parent(V2,V3)` would "cover" anything). Search is exhaustive within a body
bound and deterministic; there is no conflict-driven reasoning and no metarule machinery, and no code
from Popper or Metagol is copied.

On a six-person domain with `parent` and `same_city` facts, two positives and two negatives, the
learner returns

```
grandparent(V0,V1) :- parent(V0,V2), parent(V2,V1).
```

Projected onto the 36 ordered pairs, that extension is accepted by `metaform propose` with `splits:
2`, and `scripts/check-components.ps1` re-runs the whole path and fails if the clause or the verdict
changes.

### 7.3 The controller, and a record of what it did

`metaform-runtime run` is the loop the paper's alternation corollary prescribes: while the state is
insufficient, ask every applicable operator for a predicate, validate the proposals, adopt the one
that separates the most colliding pairs (cheapest on a tie), and stop when nothing separates anything
or the state is no longer insufficient. It never invents a proposal and never adopts one the validator
rejected.

Each step appends a line to the record file:

```
state_fingerprint  step  operator  proposal  splits  colliding_before  adopted  cost  status_before  status_after
```

On the fixture state the run takes one step: retrieval proposes `a&b`, it separates both colliding
pairs, and the state goes from `INSUFFICIENT` to `IDENTIFIED`. On a state that is already decided the
run takes no step at all, which is the property worth having: the controller does not act when there
is nothing to act on. `scripts/check-components.ps1` asserts both.

### 7.4 Experience: what the record says, and what it is allowed to change

Each record line now carries the state class as well — objects, vocabulary size, labelled examples,
colliding pairs, all taken *before* the step. `metaform-runtime experience` aggregates by class and
operator: attempts, adoptions, mean cost, and for a given state the operator that has adopted most
often in its class.

On the fixture state, two recorded runs are enough to make the class (4, 1, 4, 2) point at
`vsa-retriever`, and `run --use-record` then puts the retriever first. That is all the layer does:
it reorders the operators. Every operator is still asked, the validator still decides what is well
formed, and the controller still decides by pairs separated. The record cannot make a bad proposal
acceptable, which is the property that keeps this honest.

Unseen classes get no suggestion and the default order stands; a missing record file is an empty
record rather than an error, because the runtime has to work before it has ever run.

### 7.5 Every offer is recorded, and what that showed

The first version of the record wrote one line per *step*, so only the winning operator appeared. That
made the experience table vacuous: every operator it had ever seen had adopted 100% of the time. The
record now writes one line per **offer**, adopted or not.

`experiments/controller_runs.py` then runs the controller over the three families with both the
retrieval component and a file-backed proposer holding the model's proposals, and the aggregated
record looks like this:

| class (objects, vocabulary, labelled, colliding) | operator | attempts | adoptions | mean cost |
|---|---|---|---|---|
| (12, 1, 4, 4) | file-proposer | 15 | 15 | 0.000 |
| (12, 1, 4, 4) | vsa-retriever | 44 | 0 | 0.074 |
| (12, 1, 4, 3) | file-proposer | 5 | 3 | 0.000 |
| (12, 1, 4, 3) | vsa-retriever | 15 | 2 | 0.074 |
| (12, 2, 4, 1) | vsa-retriever | 6 | 3 | 0.074 |

Two things fall out of it, and neither was visible before the per-offer record existed:

1. **In the four-pair class the retrieval component never wins**: 44 offers, 0 adoptions, because the
   model's proposal separates the same pairs at cost zero and the tie is broken on cost. It is not
   that retrieval is useless; it is that this class gives it nothing the other arm does not also give,
   cheaper.
2. **Two documents disagree about the noisy family**: in `proposer_arms.py` the model arm scores 1.000
   on `noisy`, but in the controller two of the five noisy states are settled by retrieval. The
   difference is the metric: the arms experiment asks whether a proposal separates the pairs the
   *true* labels imply, while the controller can only see the pairs the *given* labels imply. Where a
   label is wrong, fitting it beats being right, and the controller has no way to know.

The second point is the kind of thing the tool was supposed to expose. It also says what the next
missing operator is: something that can distrust a label, which is what the probe operator in the
kernel already is for inherited classes.

### 7.6 The probe: naming a label it had to disbelieve

The controller decides by pairs separated, and that count comes from the labels it was given. When a
label is wrong, a concept that fits the true structure separates fewer pairs than a concept that fits
the mistake, so the honest proposal loses and nothing records why (7.5).

`probe-labels` answers the direct question instead: for each labelled object, if that one label were
reconsidered, would some concept separate everything? It reports one candidate per suspect, and its
candidates are ordinary proposals -- it cannot change the evidence, only say which label it had to
disbelieve.

Run on the five noisy states with the flipped label known, it names the true object in all five:

| state | truly flipped | named among the suspects |
|---|---|---|
| noisy:0 | 5 | 1, 5 |
| noisy:1 | 4 | 4, 2 |
| noisy:2 | 4 | 4, 0, 11 |
| noisy:3 | 3 | 3, 5 |
| noisy:4 | 10 | 10, 2, 6 |

Two things it does not do, and these matter more than the recall. It cannot act on a suspicion --
correcting evidence is not something a proposal may do -- so the conflict stays visible in the record
rather than being resolved. And it has false positives: on every state it names at least one object
that is labelled correctly, because a wrong label and an unusual object look the same from four
examples. The list is a question to put to a checker, not a verdict.

### 7.7 Raised, recorded, counted — and the case it misses

When the adopted proposal cannot separate every pair and the probe has named a suspect, the controller
now says so in the run (`conflict: ...`), writes a flag in the record, and `experience` counts the
steps per class. On the five noisy states that gives three flagged steps, all in the class
`(12, 1, 4, 3)`.

Three, not five, and the two missing ones are the interesting part. In `noisy:3` and `noisy:4` the
adopted proposal separates **every** pair under the labels as given -- it fits the mistake exactly --
so the controller has nothing to complain about, even though the probe had named the truly flipped
object in its suspects. The conflict flag fires when the labels are *inconsistent*, not when they are
*consistently wrong*, and from the outside those two look different only if somebody looks at the
suspects anyway.

That is a property of the mechanism, not a bug to patch away: a detector that fires on every proposal
would flag every state. What it means practically is that the probe's suspect list has to be read
even when the run reports success, which is the same lesson as `splits` being necessary and not
sufficient.

### 7.8 Two arms with a language model in the loop (pilot, n = 4)

`llm_arms.py` builds a concept-identification instance (12 objects, 8 attributes, hidden rule a
conjunction of two attributes), writes the prompts for two arms, and scores both by the same question:
does the final hypothesis equal the hidden rule?

- **D0, direct, four labels, one answer.** The model answers from the four labelled objects and may
  not ask anything. 0/4.
- **S, scaffolded, four labels plus the kernel's question.** The model proposes up to four predicates;
  the real binary scores them by separated colliding pairs, adopts the best, and `ask` picks one object;
  the model is shown the answer and the final vocabulary is scored. 4/4, with the kernel asking one
  question per instance.
- **D1, the same six labels, no loop.** A fresh model gets exactly the evidence S ended with and answers
  once. 4/4.

The honest reading is the third row: the whole gain of the loop here is the label the kernel chose, not
the proposal bookkeeping. On this generator the labels come from the target, so the smallest attribute
conjunction consistent with the evidence is usually the target, and a direct answer with the same
evidence is enough. What the loop demonstrably adds is **which question to ask**: it picks an object
inside the competing concept and outside the target, which is what turns a tie into a decision.

This is a pilot, and its limits are worth stating. n = 4; the two arms were run by different model
instances; the task family is easy enough that a simplicity heuristic finds the target; no budget is
charged for prompt tokens. The design that would test anything is a matched-evidence comparison at
larger n on a family where the minimal conjunction is not the target.

A first probe on real code suggests where the loop is and is not needed. Three mutations in this
repository: changing a report heading failed no test; changing the vote threshold failed two tests,
both in the same module; dropping the asked-mask update failed one. In all three the failing set lies
inside a single module, so a one-feature vocabulary decides it and the kernel reports `IDENTIFIED`
without proposing anything. The loop's case is a change whose failures cross modules, where the static
vocabulary collides and a question buys the missing distinction.

## 8. Component library survey and the capacity of an HDC memory (2026-10-20)

Four clones were read before writing the vector component, so the port copies what exists instead of
re-deriving it: the TorchHD core, Popper, the Cognitive Rule Engine, and the `hypervector` crate this
component was originally adapted from.

What the reading changed:

1. **`hypervector` cannot be the base.** Its FHRR stores phase angles, so every cleanup pays one `cos`
    per element: measured 3.9 s for a 10^5-item query at d = 10^4, against 0.2 s for bipolar vectors.
    It has no unbind, no permute, no item or associative memory, and its global `Mutex<StdRng>` defeats
    per-session determinism; it also pulls 74 transitive crates through an unused plotting dependency.
    The fork therefore keeps the bipolar core only, and the missing layer was written here.
2. **TorchHD has no capacity formula anywhere in the source.** Its memory is a linear cosine scan with
    a threshold; role-filler is `multiset(bind(role, filler))`; order is `sum Pi^(n-1-i)(x_i)`. Those
    three constructions are what the Rust encoding implements.
3. **Popper has no general theta-subsumption.** Clause subsumption is a Prolog `subsumes/2` used only
    to delete redundant literals; the workhorse redundancy test is *coverage* subsumption on bitsets,
    and refinement is compiled into ASP rather than applied as a literal-addition operator. A Rust
    learner should copy the coverage bitsets and the canonical program identity, not the search.
4. **The Cognitive Rule Engine has no SMTB.** Its `antiunify` is a masked copy of the left condition:
    literals are bucketed by key, variables aligned greedily with an upper-bound DFS, and literals with
    differing constants are dropped rather than generalised. That is the algorithm worth porting for
    structure mapping, and it is smaller than the textbook LGG it replaces.

The measured capacity of the component (`cargo run --release -p metaform-vsa --bin hdc_bench`,
top-1 over 64 probes, D = 4096, raw-trace recall):

| pairs | clean | 5% flips | 10% flips | 20% flips |
|---|---|---|---|---|
| 16 | 1.000 | 1.000 | 1.000 | 1.000 |
| 64 | 1.000 | 1.000 | 1.000 | 0.984 |
| 256 | 0.906 | 0.797 | 0.625 | 0.312 |
| 1024 | 0.094 | 0.078 | 0.047 | 0.000 |

Quantising the trace before cleanup costs about a third of the capacity at 256 pairs (0.672 against
0.906); keeping the integer accumulator is not an optimisation but a correctness decision. The numbers
follow the `1/sqrt(N)` signal law and fail where a top-1 decision over N candidates needs a margin
larger than the noise maximum. At D = 4096 the usable point is about 128-256 pairs, not 10^5: a claim
of 10^5-item recall needs a larger dimension, sharded traces, or a different addressing scheme, and
until one of those is measured the library promises only the table above.

## 9. The relational necessity criterion (2026-10-20)

The kernel's collision test is unary: it says no concept over the current attributes separates two
objects. The relational version asks the same question about patterns: is there a conjunctive query
over the relations that holds of one object and not the other?

**Fragment.** Tree-shaped patterns: a body grown by attaching one literal at a time, one distinguished
variable at the root. This is exactly what literal-addition refinement produces, so the criterion and
the learner speak the same language. General conjunctive queries are *not* decided here: that is a
homomorphism test, NP-hard in general.

**Decision procedure.** Colour refinement. Round 0 is the unary signature; each later round adds the
multiset of (relation, position, neighbour colour). Refinement only splits classes, so it stops when
the class count stops growing. Equal colours mean no separating pattern exists in the fragment;
different colours mean one does, and the refinement history yields it.

**Witness.** Built from the class description, then greedily minimised by dropping literals while the
clause still holds of one object and not the other. On the demo domain (0 and 1 both red; 0 links to
green 2, 1 links to blue 3) the witness is

    sep(V0) :- link(V0,V1), green(V1).

and the bisimilar pair (two red objects, each linking to a green object) returns `None`. That `None`
is the point: it is the relational form of "the vocabulary is insufficient", and no amount of the
same relations will fix it.

**What Popper does not have.** Its only subsumption is a Prolog query used to delete redundant
literals, and its redundancy test is coverage on bitarrays; there is no general clause-to-clause
theta-subsumption in the source. A necessity argument over clauses needs three pieces, and all three
are now here: theta-subsumption with injective matching, a coverage bitset, and literal-addition
refinement as an ordinary function a runtime can call.

## 10. SME4's evidence arithmetic (2026-10-20)

The SME4 source was read before touching the structure mapper. Its shape: expressions are
(predicate . args) with entities created implicitly from argument positions; local matches are only
expression-by-expression, bucketed by identical functor (entity-by-entity is never proposed); a local
match is worth 5.0e-4, and each parent receives eight times the evidence of each child. Conflicts are
computed as nogoods shared through descendants, so one-to-one and no-cross-mapping are emergent
rather than checked at proposal time. Merge is greedy over kernels in descending score, accepting a
kernel only if it keeps at least 0.8 of the best score and at most 3 mappings are kept. Candidate
inferences come only from unmatched top-level source expressions and stand for unmapped entities as
skolems.

Calibration from the corpus, not from taste: in the geometry runs an entity that is the argument of
nine matched expressions scores exactly 9 x 8 x 5.0e-4 = 0.036, and an eight-parent entity 0.032.
The port now carries those constants (`LOCAL_EVIDENCE`, `TRICKLE_DOWN`) and an `evidence()` function,
with tests pinning both numbers. The structural counters (matched, systematicity, components) stay
alongside: one number would hide whether an alignment is deep or merely wide.

Adopted: local/8x arithmetic, bottom-up evaluation, entity evidence as the sum over parent matches.
Not yet adopted: symmetric and commutative twin matches, nogood propagation inside the merge, the
0.8 cutoff with at most three mappings, and skolem-marked candidate inferences. Those are the next
three commits, in that order.

### 10.1 Twin matches and the greedy cutoff (same day)

Two of the missing pieces are in. A symmetric relation now gets a twin match with the arguments
reversed: the flag lives on the system (`push_symmetric`), because a relation name cannot say whether
argument order carries meaning. The merge also drops alignments that score below 0.8 of the best, the
cutoff SME4 uses. Two tests pin the twin: present for `parallel`, absent for `above`.

Two limits are stated rather than hidden: the twin is generated at the level of the symmetric
statement itself (nested statements are matched by their own flags), and commutative n-ary order is
still not modelled. Open: nogood propagation inside the merge, skolem-marked candidate inferences.

### 10.2 Skolem-marked candidate inferences (same day)

SME4 projects an unmatched source expression even when it mentions an entity the target does not
have, writing that entity as a skolem. The new `candidate_inferences_with_skolems` does the same:
skolem ids start at `target.entities`, so a caller can tell one from a real entity by the bound, and
`skolems` records which base entity each one stands for. The old function stays, and a test shows the
difference: it drops the statement, the new one marks it. Still open from SME4: nogood propagation
inside the merge, commutative n-ary order, and a golden comparison against a dehydrated `.sme` pair.

## 11. A hand-driven session on a real question (2026-10-20)

**Question.** The repository pins the Rust pass count in two scripts and repeats the number in both
READMEs. If that number moves off 124, which tracked files must be edited?

This is a maintenance question about the repository itself, not a constructed domain.
`scripts/check-rust.ps1` defaults `-Expected` to 124; `scripts/check-all.ps1` forwards its own
`$RustExpected` default; `README.md` and `README.zh.md` state the same number in their command lists.
`git grep -n --fixed-strings '124'` finds exactly five tracked files, so the answer is decidable
without running a test. The pin is 124 at commit `3eb0d7d`. The answer is the four count-bearing
files; `Cargo.lock` is the fifth hit, but its 124 is inside a checksum.

**Objects.** Seven tracked files, ids 0 to 6. The set contains all five files that `git grep` finds
for `124`, plus two count-adjacent scripts. Three are labelled positive, three negative; one is
deliberately left unlabelled.

| id | file | label | why it is in the set |
|---|---|---|---|
| 0 | `scripts/check-rust.ps1` | positive | owns the default pin |
| 1 | `scripts/check-all.ps1` | positive | forwards the pin to check-rust |
| 2 | `README.md` | positive | repeats the number in prose |
| 3 | `README.zh.md` | unlabelled | true positive by grep, withheld from the evidence |
| 4 | `scripts/check-lean.ps1` | negative | a script with no count |
| 5 | `scripts/selftest-gates.ps1` | negative | mentions `Expected`, but as `-Expected 999` |
| 6 | `Cargo.lock` | negative | 124 occurs in a checksum, not a count |

**Initial vocabulary.** Set by hand with `state add`; no predicate mentions 124.

| name | ids | computed as |
|---|---|---|
| `is_script` | 0,1,4,5 | the four `scripts/*.ps1` objects |
| `is_doc` | 2,3 | the two README objects |
| `mentions_Expected` | 0,1,5 | `Select-String -CaseSensitive 'Expected'` hits 0, 1 and 5; 5 is `-Expected 999` |

`mentions_Expected` is the plausible-but-wrong predicate: it puts positive 0 and 1 in the same
signature class as negative 5, so it creates the collision instead of resolving it.

**Labels and the grep that establishes each.** All commands were run at the repository root;
`Select-String` is the PowerShell grep. The positive criterion is "contains the literal pinned
count"; the negative criterion is "does not carry it as a count".

- 0 positive. `Select-String -Path scripts/check-rust.ps1 -Pattern '124' -SimpleMatch` returns
  `17: Expected pass count (default 124: 70 kernel + ...)` and `26: [int]$Expected = 124,`. The
  default is the pin itself, so the file must move with it.
- 1 positive. `Select-String -Path scripts/check-all.ps1 -Pattern '124' -SimpleMatch` returns
  `24: Expected Rust pass count (default 124).` and `35: [int]$RustExpected = 124,`. The script
  passes `-Expected $RustExpected`, so it must move too.
- 2 positive. `Select-String -Path README.md -Pattern '124' -SimpleMatch` returns
  `212: ... # Rust only (pass count must == 124)`. The prose would become false.
- 4 negative. `Select-String -Path scripts/check-lean.ps1 -Pattern '124','Expected' -CaseSensitive`
  returns no output. It runs Lean, not the Rust count.
- 5 negative. `Select-String -Path scripts/selftest-gates.ps1 -Pattern 'Expected' -CaseSensitive`
  returns `62: ... check-rust.ps1') -Expected 999 | Out-Null`; the same file with
  `-Pattern '124' -SimpleMatch` returns no output. The 999 is intentionally wrong for the negative
  self-test; it does not track the pin, so changing 124 to N does not require touching it.
- 6 negative. `Select-String -Path Cargo.lock -Pattern '124' -SimpleMatch` returns
  `32: checksum = "65cd...1240a"`; `Select-String -Path Cargo.lock -Pattern 'test result|Expected|pinned' -CaseSensitive`
  returns no output. The match is inside a hash, not a count.

The unlabelled file, used only to audit the proposed extension and not as a label:
`Select-String -Path README.zh.md -Pattern '124' -SimpleMatch` returns
`169: ... 只跑 Rust（通过数必须 == 124）`. It is positive by the same criterion; the state was not
told so.

**Session.** `metaform init` creates the project skeleton, `state init` writes the state,
`state add` builds the vocabulary, and `state vote` writes the six labels. The loop then runs
`state need`, `suggest`, `suggest --emit-request`, `propose`, `apply`, and `state need` again.
Flags were checked against `engine/src/main.rs` and the command list in `README.md`; no flag was
guessed. All commands ran from the repository root and all exited 0. The project path is relative;
the scratch project lives under `dist/session-2026-10-20/`.

```text
PS> target\debug\metaform.exe init --project dist/session-2026-10-20/project
{"ok":true,"dir":"dist/session-2026-10-20/project/.metaform"}
exit=0

PS> target\debug\metaform.exe state init --project dist/session-2026-10-20/project --n 7
{"ok":true,"n":7,"file":"dist/session-2026-10-20/project/.metaform/state.json"}
exit=0

PS> target\debug\metaform.exe state add --project dist/session-2026-10-20/project --name is_script --ids 0,1,4,5
{"ok":true,"name":"is_script","ids":[0,1,4,5]}
exit=0

PS> target\debug\metaform.exe state add --project dist/session-2026-10-20/project --name is_doc --ids 2,3
{"ok":true,"name":"is_doc","ids":[2,3]}
exit=0

PS> target\debug\metaform.exe state add --project dist/session-2026-10-20/project --name mentions_Expected --ids 0,1,5
{"ok":true,"name":"mentions_Expected","ids":[0,1,5]}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 0 --yes 3 --no 0
{"ok":true,"id":0,"yes":3,"no":0}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 1 --yes 3 --no 0
{"ok":true,"id":1,"yes":3,"no":0}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 2 --yes 3 --no 0
{"ok":true,"id":2,"yes":3,"no":0}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 4 --yes 0 --no 3
{"ok":true,"id":4,"yes":0,"no":3}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 5 --yes 0 --no 3
{"ok":true,"id":5,"yes":0,"no":3}
exit=0

PS> target\debug\metaform.exe state vote --project dist/session-2026-10-20/project --id 6 --yes 0 --no 3
{"ok":true,"id":6,"yes":0,"no":3}
exit=0

PS> target\debug\metaform.exe state need --project dist/session-2026-10-20/project
{"status":"INSUFFICIENT","action":"PROPOSE","theta":3,"epsilon_hat":0.00,"theta_rec":3,"classes":4,"free_classes":0,"colliding_pairs":[[0,5],[1,5]],"pending":[],"unverified":[3],"suggested_labels":[]}
exit=0

PS> target\debug\metaform.exe suggest --project dist/session-2026-10-20/project --max-arity 2 --max 20
{"status":"INSUFFICIENT","colliding":2,"reason":"boolean_recombination_cannot_separate","colliding_pairs":[[0,5],[1,5]],"next":"new_atom","tried":9,"suggestions":[]}
exit=0

PS> target\debug\metaform.exe suggest --project dist/session-2026-10-20/project --emit-request
{"protocol":"metaform.propose/1","request_id":"suggest-70569a53b49bd339","objects":7,"object_ids":[0,1,2,3,4,5,6],"vocabulary":[{"name":"is_doc","ids":[2,3]},{"name":"is_script","ids":[0,1,4,5]},{"name":"mentions_Expected","ids":[0,1,5]}],"evidence":{"pos":[0,1,2],"neg":[4,5,6]},"theta":3,"colliding_pairs":[[0,5],[1,5]],"allowed":{"ids":true,"define":{"ops":["all_of","any_of"],"operands":"vocabulary"}}}
exit=0

PS> target\debug\metaform.exe propose --request dist/session-2026-10-20/request.json --response dist/session-2026-10-20/response.json
{"protocol":"metaform.propose/1","request_id":"suggest-70569a53b49bd339","accepted":[{"name":"contains_pinned_count","ids":[0,1,2,3],"splits":2,"warrant":"Select-String finds 124 at check-rust L17/L26, check-all L24/L35, README L212 and README.zh L169; Cargo.lock L32 is a checksum, not a count, and is excluded","falsifier":"a file that must move with the pin but contains no 124, or a file whose 124 is incidental (Cargo.lock is the substring trap)"}],"rejected":[],"refused":null}
exit=0

PS> target\debug\metaform.exe apply --project dist/session-2026-10-20/project --request dist/session-2026-10-20/request.json --response dist/session-2026-10-20/response.json
{"applied":1,"skipped":[],"before":{"status":"INSUFFICIENT","colliding":2},"after":{"status":"IDENTIFIED","colliding":0},"request_id":"suggest-70569a53b49bd339"}
exit=0

PS> target\debug\metaform.exe state need --project dist/session-2026-10-20/project
{"status":"IDENTIFIED","action":"VERIFY","theta":3,"epsilon_hat":0.00,"theta_rec":3,"classes":5,"free_classes":0,"colliding_pairs":[],"pending":[],"unverified":[3],"suggested_labels":[]}
exit=0
```

**One proposer turn.** The request is 406 characters. It allows both an extensional `ids` candidate
and a `define` over the three existing predicates. No definition can separate the pair: objects 0,
1 and 5 all have the signature `(is_script=T, is_doc=F, mentions_Expected=T)`, and a Boolean
combination is constant on a signature class. `suggest` tried all nine definitions (three
singletons, three `all_of`, three `any_of`) and found none, which is Theorem 11's negative result
rather than a truncated search.

The response therefore supplies one extensional atom. Its full text is:

```json
{"protocol":"metaform.propose/1","request_id":"suggest-70569a53b49bd339","candidates":[{"name":"contains_pinned_count","ids":[0,1,2,3],"warrant":"Select-String finds 124 at check-rust L17/L26, check-all L24/L35, README L212 and README.zh L169; Cargo.lock L32 is a checksum, not a count, and is excluded","falsifier":"a file that must move with the pin but contains no 124, or a file whose 124 is incidental (Cargo.lock is the substring trap)"}],"refused":null}
```

The extension is 0, 1, 2, 3: the two scripts that pin or forward the count, both READMEs, and not the
two count-adjacent scripts or the checksum. It covers unlabelled object 3 (README.zh) on the strength
of the same grep that established the other positives.

**Verdict sequence.** One proposal, one proposer turn, status `INSUFFICIENT` to `IDENTIFIED`:

1. `state need`: `INSUFFICIENT` / `PROPOSE`; 4 classes, 2 colliding pairs `[0,5]` and `[1,5]`;
   `unverified` is `[3]`.
2. `suggest`: `reason: boolean_recombination_cannot_separate`, `next: new_atom`, `tried: 9`,
   `suggestions: []`.
3. `suggest --emit-request`: `request_id: suggest-70569a53b49bd339`; `allowed` admits `ids` and
   `define`.
4. Proposer: one extensional candidate, `contains_pinned_count = [0,1,2,3]`.
5. `propose`: accepted 1, `splits: 2`, rejected 0, refused null.
6. `apply`: applied 1; before `INSUFFICIENT` / 2, after `IDENTIFIED` / 0.
7. `state need`: `IDENTIFIED` / `VERIFY`; 5 classes, no colliding pairs; `unverified` is `[3]`.

**Convergence and cost.** The status converged on the first proposal: the vocabulary now separates
the positive and negative classes, so the loop stopped. It did not fully verify: the action is
`VERIFY`, because README.zh (3) was never asked directly and its label is inherited from README.md
in class `{2,3}`. The run stopped on the task's rule (status `IDENTIFIED`), not because every object
had a direct vote. The proposal cost one turn and one command pair; there was no retry, no second
candidate, and no `suggest` loop.

No tokenizer was attached to the session, so the token figures are byte-based estimates at roughly
3.5 bytes per token. The proposal turn saw the 406-byte request plus about 2 KB of grep evidence and
file lines, roughly 1k input tokens, and emitted the 462-byte response, roughly 150 output tokens
including the reasoning that ruled out `define`. The whole session, including reading `README.md`
and `engine/src/main.rs` to verify the flags, was on the order of 15k to 20k input tokens; that
reading is shared setup, not a per-proposal cost.

**What this shows, and what it does not.** This is n=1, hand-driven, and there is no model API in the
loop: `metaform` never called a model, the proposer was this same agent, and the labels, the initial
predicates, and the proposed extension were all chosen by that one agent, so the evidence and the
proposal are not independent. It shows that the plumbing carries a real distinction end to end on
repository content: `suggest` proved that the plausible `mentions_Expected` predicate cannot separate
the pair, the request was well-formed, the validator accepted exactly one candidate with
`splits: 2`, `apply` changed the status, and a subsequent `state need` agreed with the validator.
It also shows a real instance of the failure mode the loop is about: the existing vocabulary had a
count-adjacent predicate and still could not express "contains the pinned count"; the missing atom
was a property of the repository, not a Boolean recombination. It does not show that a model would
find that atom, that the atom is minimal or stable, that the labels are correct beyond the greps
shown, or anything about proposal cost, convergence rate, or label quality at n > 1. The engine
checks that the candidate is non-constant, in range, and splits the colliding pairs; it cannot check
that `contains_pinned_count` means what its name says. A candidate `[0,1,2,3,6]`, which includes the
checksum file, would also have been accepted with `splits: 2` and would have left the same
`IDENTIFIED` status. Excluding `Cargo.lock` is the proposer's semantic judgment, not the engine's,
and the same holds for including the unlabelled README.zh. So the pilot shows more than "the plumbing
works" only in the narrow sense that the insufficiency report was informative on a real question; it
is not evidence that the loop is cheap, reliable, or model-findable.

## 12. A second proposer arm, and why it does not count as blind (2026-10-20)

The plan was a controlled comparison: the first arm knew the labels and proposed; a second arm
would see only the request and the repository, with the evidence ledger off limits. The second arm
was run, and it was not blind. Three channels leaked the answer before it proposed:

1. a repository-wide grep for the predicate names matched the evidence ledger, and the grep output
   itself carried the accepted proposal;
2. the session transcript holds the first response verbatim, and it sits beside the request;
3. the project state already contains the accepted predicate, because the first arm adopted it.

The arm is therefore not evidence about findability. It is still useful evidence about two other
things. It rebuilt the same extension from file facts alone and named the predicate differently
(pins_rust_pass_count), and the engine accepted it with splits two on a clean rebuild whose request
id matched. And it found a distinction the first arm had not needed to state: the property is
"carries the pinned count", not "contains the number" -- the lock file's 124 is inside a checksum,
so a substring predicate would wrongly include a negative.

What a blind arm requires, written down so the next attempt does not repeat the mistake: a fresh
project built from a written specification rather than from the post-adoption state, a request
emitted before any proposal exists, no ledger or transcript in the tree the proposer can reach, and
a grep that excludes the evidence directory. Only then is the turn count and the hit rate a
measurement rather than a demonstration.

## 13. The request publishes the answer (2026-10-20)

Before running a truly blind arm, the environment was rebuilt outside the repository and checked
for leaks. The leak that mattered was not in the directory: it was in the protocol. A proposal
request carries `evidence.pos` and `evidence.neg`, and those ids are the extension of the target
concept. A proposer that reads no file at all can answer `ids = 同 evidence.pos` and be accepted with
two pairs split. Verified: such a candidate is accepted, and it is exactly the shape of the first
arm's successful proposal.

So a request-based experiment measures copying, not discovery. The hidden part is only the intension
-- the name and the warrant that explain why those files and no others -- and the current protocol
scores the extension, not the intension. Two consequences:

1. Any "blind arm" built on this request shape is invalid, however clean the directory. The previous
   section's arm was contaminated by artifacts; a clean arm would still carry this.
2. The measurement needs a held-out design: label a subset of objects, withhold the rest, and score
   whether the proposed predicate classifies the withheld ones. Then echoing the evidence fails,
   because the withheld labels are not in the request.

What the environment preparation did establish, and can be reused: the experiment runs outside the
repository, the request is emitted before any proposal exists, the object-to-file mapping is given
to the proposer instead of guessed, the state holds only the three insufficient predicates, and the
smoke test (a deliberately wrong candidate) shows the engine accepting a candidate that splits one
pair -- evidence that validation checks consistency, never meaning.

One further leak is worth recording because it is easy to miss: the answer is in the tracked history.
Hiding the working-tree directories does not hide it, since the ledger is tracked and `git show HEAD`
returns it.

## 14. A held-out split is not enough if the vocabulary contains the answer (2026-10-20)

The first held-out harness labelled four of eight objects and withheld the rest, so that echoing the
evidence would fail. It does fail: the evidence-echo scores two misses on the withheld positives. But
the split does not survive contact with the rest of the request, for two reasons that are about
design, not about the proposer.

1. One vocabulary atom is the withheld set. The atom "is a document" has extension {2,3}, and those
   are exactly the withheld positives. A proposer that reads only the request can take the positive
   evidence and union it with that atom, and score perfectly. Verified: the union candidate scores
   two true positives, no errors.
2. The object numbering encodes the answer. Positives are 0-3, negatives 4-7, so a guess based on
   the id prefix reaches the same score with no reasoning at all.

The lesson generalises: a held-out test is blind only if (a) no published atom coincides with the
withheld set or its complement, (b) object ids are shuffled so the answer is not an interval, and
(c) the withheld set is not recoverable as a short Boolean combination of published atoms -- which is
the same theorem the engine uses, applied to the experiment.

Also recorded because it is easy to miss: the scoring script hardcodes the withheld truth, so it
must never be reachable by the proposer; the experiment directory, the working-tree session
directories and the tracked history all contain the answer from earlier arms; and a proposer that is
a fork of the parent session inherits the truth map, so it must be a fresh context with only the
brief and the request.

Nothing in this section invalidates the earlier finding that the request publishes the evidence. It
sharpens it: the request publishes the evidence and the vocabulary, and a target concept that is a
short combination of the two is published as well.

## 16. A held-out experiment with an independent proposer (2026-10-21)

Question, chosen because its answer is stated nowhere in the tracked tree or the history: which of
these files belong to a workspace crate that declares at least one dependency on a crate not built
from this repository (a version requirement, not a path).

Twelve objects: all seven workspace manifests plus five decoys. Five atoms computed from file
content. Four positives, eight negatives; five labelled (two positive, three negative), seven
withheld (two positive, five negative). Ids shuffled, no interval and no prefix pattern.

The three leak checks were run exhaustively before the arm: no single atom equals the withheld set
or its complement; of 35 single/union/difference expressions at most two atoms wide, none equals it
(55 when intersections and symmetric differences are added); of 45 atom-plus-or-minus-evidence
expressions, none equals it. All 62 all_of/any_of expressions over the vocabulary contain neither
the withheld set nor the positive set, so only an explicit ids candidate can be exact. Enumeration
and split search are in the experiment directory under tmp, which is not tracked.

Two attempts. The first was invalid: the environment was rebuilt after the proposer read it, so the
response carried a stale request id and the kernel rejected it as wrong_request_id. Its extension
happened to score exact against the final truth, but the ids referred to different files, so it is
not a measurement. The second attempt read a frozen environment; its request id matched byte for
byte, and the result is the one below.

Held-out score: tp 4, fp 0, fn 0, tn 8, exact. The proposer returned a single ids candidate
whose extension is exactly the four positives, with a prediction on id 5, judged negative, which is
correct. One proposal, one turn.

The kernel rejected that response: refused was written as the boolean false rather than null or a
string, so the validator answered malformed_response. The concept was right and the file was
invalid, and the protocol caught it. A reviewer would have fixed the field without noticing.

What this does not show. It is one question, one proposer, one turn, no budget control, and the
proposer is a subagent with repository read access rather than a metered model API; token counts are
therefore absent rather than measured. The exact extension is reachable by reading the manifests and
the lock file, which is the intended reasoning path, so this is evidence that the task is solvable
from the corpus, not evidence that the engine caused the solution. One positive had been withheld
precisely because its own source file stated its dependency situation; that object was replaced
before this run, and the replacement was checked for the same defect.

The lesson that cost two attempts: freeze the environment before dispatching a proposer. The first
attempt failed for no reason the proposer could control, and a rebuild between read and score
invalidates a run even when its numbers look right.

## 17. The loop through the host (2026-10-21)

The server is installed outside the repository, registered in the host's profile as an inserted
plugin, and its ten tools are visible to the model in the same session. The following ran through
those tools, not through the command line, on the repository's own demonstration state.

The request carried twelve objects and one predicate, with object 6 positive, object 2 negative,
and both outside that predicate; the two collided. The proposal was a singleton extension, object 6
alone, with a prediction that object 9 is negative. It was accepted: the status moved from
INSUFFICIENT to UNDETERMINED and the colliding count fell to zero. The prediction was recorded with
the other open one; a vote labelling object 9 negative was then cast, and the check reported one
prediction checked, one correct, none open.

Exploration then asked whether the existing predicate implies the new one. It does not: objects 3,
4, 7 and 10 have the predicate and are not object 6. The answer was given as a refutation with
object 3 as the counterexample. The laws file gained one line whose kind is refutation, not law, and
a second exploration asked nothing.

What this shows: the wire works end to end; a prediction is recorded before it can be checked and
scored after; and a negative answer is stored as a refutation rather than as the implication it
denies. What it does not show: the accepted candidate is a lookup table, a constant extension chosen
to separate one pair, which is exactly the invention the capacity bound does not price; the
proposer had read the demonstration ledger, so the correct prediction was not blind; and the domain
is the repository's own fixture rather than a task. This is a channel test, not a measurement.
