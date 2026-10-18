# 06 The AI-native hypothesis: removing "the human" from the cost model

The original setup inherited the posture of FCA attribute exploration: humans supply attributes, humans answer membership questions. This system is meant for LLM/AI use,
so that assumption has to be replaced. After replacing it, some conclusions grow stronger and some have to be reformulated.

## 1 Three replacements

| Old assumption | New assumption | Consequence |
|---|---|---|
| the oracle is a human, each answer is expensive | the oracle is a program / test / Lean / the model itself; **answers are cheap and parallelizable** | questions are no longer the scarce resource, **proposals are** |
| one question at a time, one answer at a time | one model call can answer hundreds of membership questions | the cost unit shifts from "one query" to "**one round (batch)**" |
| the user is a human looking at a concept lattice | the user is an agent reading structured state | interfaces must emit JSON, not tables meant for humans |

## 2 Which conclusions are unchanged

- **T2 / T6 / T7 are completely unchanged**: they talk only about vocabulary and signatures, regardless of who answers.
  A collision is a collision, whether the oracle is a human or a compiler.
- The paper's criterion is therefore firmer: it does not rely on the sociological premise that "experts are willing to cooperate."

## 3 What has to change

**T5's lower bound must be counted per round.** If one round can ask about k objects at a cost of 1 per round, then identifying among |F| candidates still needs
`k · rounds ≥ log2 |F|` bits, but **rounds ≥ ⌈log2|F| / k⌉**.
Engineering implication: rather than designing "who to ask next," design "**which k to ask this round**" — filling each round with one representative of every undecided class
is the optimal batch policy (domain A has shown that which member within a class is asked does not matter).

**The cost model flips.** Let one proposal cost `r` (generation + verification), one round of batch queries cost `c`, and the hit rate be `q`.
Total cost `≈ (r/q) + c·rounds`. In domain A / the cost experiments `r/q` dominates (a 44× span; stopping rules differ by only ±6%) —
under the AI setting this conclusion only gets stronger, because `c` is smaller than `r`.

**Self-adjudication must be forbidden.** If the oracle and the proposer are the same model, answers and proposals share the same source of error
(TACL 2024 survey: no evidence that prompt feedback alone enables self-correction). The workflow therefore requires: hand the decidable parts to a
program (tests, Lean, execution), and the undecidable parts to **another** model or a human.

## 4 Requirements on the engine interface

An agent should not read tables; it should read JSON. `qde need` should output:

```json
{
  "status": "INSUFFICIENT",
  "classes": 2,
  "free_classes": 0,
  "colliding_pairs": [[0,4],[0,5],[1,4],[1,5]],
  "suggested_labels": []
}
```

and `qde batch k`: give the k objects to label in this round (one representative per undecided class), asked all at once.

Once the agent receives `colliding_pairs`, what it has to do is clear: **generate predicates that split those pairs**,
rather than "think harder." This is T7 turned into an interface.

## 5 What this means for myself (this agent)

I am not the operator of this system; I am one of its users. Usable postures:

1. When analyzing a codebase: objects = cases/functions, predicates = executable features, collision pairs = two cases that behave the same but lead to different conclusions.
   `need` directly tells me "these two cases are identical under every existing feature; a feature must be replaced."
2. When reading papers: objects = papers, predicates = method features, collision pairs = two papers that look different but receive the same judgment —
   which is exactly where I am most likely to err when doing literature synthesis.
3. When running experiments: objects = configurations, predicates = metric thresholds, collision pairs = configurations with the same metrics but different outcomes.

I have done all three, and always by feel. The engine's value is replacing "by feel" with "the state machine says it is insufficient, so replace the feature."

## 6 One restraint

Do not treat "used by AI" as "can be called without limit." The proposal cost r in a real system is long context + verification,
and a round of several hundred membership queries is not free either. **The cost model must be kept; it is merely switched to tokens / call counts.**
Without cost there is no question of "should we expand the vocabulary," and the whole paper degenerates into "try a few more features."

## 7 Oracle return values must carry vote counts

A model used as an oracle errs, and errs non-independently. The synthetic experiments (`RESULTS.md` §3.10) quantify:

| Per-answer error rate ε | r=1 false trigger / miss | r=5 false trigger / miss |
|---|---|---|
| 0.05 | 4.0% / 0.0% | 0.0% / 0.0% |
| 0.10 | 10.0% / 1.7% | 0.5% / 0.0% |
| 0.20 | 9.0% / 18.6% | 0.5% / 0.0% |
| 0.30 | 12.0% / 20.5% | 8.0% / 8.2% |

Therefore `Oracle::ask` should not return `Option<bool>`; it should return `(yes: u8, no: u8)` or a calibrated
confidence; **critical decisions (the one that determines whether to change the vocabulary) must be asked repeatedly**, while non-critical ones can be asked once.

This also explains why "ask again" is not expensive under the AI setting — it is cheaper than proposing, and it can push the false-trigger rate down by an order of magnitude.

**Already implemented** (`engine/src/lib.rs`): `Explorer::answer_votes(x, yes, no)` + `status_with(theta)`,
where `theta` is the vote margin required for "confirmation"; when `theta` is larger, noisy votes are no longer treated as collisions, and the status falls back to `Undetermined` instead of producing a false positive.

```
$ qde vote 0 5 1      # obj 0: five yes votes, one no
$ qde need 6          # require a vote margin ≥ 6 for confirmation
{"status":"UNDETERMINED","action":"ASK","theta":6,...,"colliding_pairs":[],...}
```

The same data would report `INSUFFICIENT` under `theta=1` but not under `theta=6` — that is how the noise gate works.

**Default θ=3**, based on the full-loop experiments (`RESULTS.md` §3.11):

| ε | θ=1 | θ=3 | θ=5 |
|---|---|---|---|
| 0.1 | 0.25 | **0.98** | 1.00 |
| 0.2 | 0.07 | 0.83 | **0.99** |
| Rounds (ε=0.1) | 5.7 | 15.6 | 22.1 |

Empirical rule `θ ≈ 2 + 10ε`: use 3 for ε≤0.1, 4–5 for ε≈0.2. Under noise, θ=1 confirms early and hands back a wrong extension;
this is an order-of-magnitude difference, not a finishing detail.

**Adaptive θ was tried and rejected** (`RESULTS.md` §3.12): at ε=0.1 it spent 44% more rounds for a 0.01
gain in success rate, and at ε=0.2 it saved 11% of rounds but reached a success rate of 0.94 < 1.00. The two fixed values bracket the adaptive one,
so we switched to "estimate ε → pick θ directly," with no online adjustment.

Estimating ε is cheap: the proportion of minority votes is a direct estimate of ε (the mean of `min(yes,no)/(yes+no)`).
**Already implemented**: `Explorer::implied_epsilon()` and `recommended_theta()` (≤0.1→3, ≥0.2→5, 4 in between),
and `qde need` directly outputs `epsilon_hat` and `theta_rec`.

**Validation** (`RESULTS.md` §3.13): for ε≤0.2, choosing θ from the estimate differs from using the true value by ≤1 percentage point, and at ε=0.2 it is also 15% cheaper in rounds; at ε=0.3 the gap reaches 7 points. So the applicable range of this self-calibration is **ε ≤ 0.2**; beyond that one has to switch to a per-object posterior,
and cannot rely on tuning the threshold.

**But the per-object posterior also failed** (§3.14): a Beta gate with δ=0.05 is slower than θ=5 at every level, and worse still at ε=0.3.
The reason is that adding votes is itself exposed to noise — the longer the round count, the more chances to be overturned. **The noise line ends here**:
going further requires "asking better" or "modeling the noise," not more threshold tuning.
