# 13 Six Readings: One 1.md, Six Different Conclusions

6 independent agents read the same 1.md (2135 lines, 2026-10-19 05:18 version), each given only one lens,
none able to see the others, and none of them having read the other documents in this repository. The
purpose is not to vote, but to see how many **mutually incompatible** readings the same text can grow.

Result: **only two of the six readings are genuinely incompatible**; the rest of the differences are a
division of labor. One more conclusion directly rejects the very phrase "brain-like".

## 1. Six lines

| Lens | One-sentence conclusion | The minimal experiment it proposes | What it finds most suspect |
|---|---|---|---|
| Logic / type theory | the unified framework = multi-judgment natural deduction (Γ⊢ₘJ) + reflection; operators = rule schemas, composition = cut | swap the representation under the same operator set; the candidate set should be **isomorphic** | whether S is "discovery" or "verification"; lines 1104 and 625 contradict each other |
| Engineering / systems | the kernel recognizes only `Hyp{lang,body}` and `score(h,s)`; the four operators are just four values of `lang` | `core.py` < 200 lines: 6 elements, 4 attributes → ≥2 distinct S, and injecting a counterexample lowers the ranking | the anti-pattern `current_best.json`: an overwriting write lets a counterexample vanish once absorbed |
| ML / inductive bias | the bottleneck is identifiability, not compute; 1.md writes E⁻ but has **not a single real counterexample** | A bare LLM / B engine / C with scoring removed / D with the scorer replaced by an LLM; the four conditions separate "the engine" from "spending more budget" | meta-learning and intent inference both need a repeatable likelihood, and here n=1 |
| Cognitive science / brain-like | **"brain-like" is the wrong goal**: the brain too stops at the first plausible explanation; what you want is superhuman auditability | four examples sharing S, measuring near neighbors N / far transfer F / decoys L; the criterion is the **interaction** ΔF>0 and ΔL≤0 | copying the brain will reproduce the very failures you complain about (confirmation bias, stopping at the first explanation) |
| Skepticism | the three claims are reskinning; unification reaches only the API layer | pre-register 50 cases, each hiding one correct member that was not mentioned; the top-1 hit must be significantly above a bare LLM (p<0.05) | evaluation undecidable (45%) > merely a reranker (30%) > unbounded expansion (25%) |
| Cross-disciplinary | the paradigms of six disciplines collapse into the same 5 classes (see §8); the original example is actually the **psychometrics** single-factor problem | factor analysis on the 13 preferences: can a single factor explain most of the variance | that agent timed out; I filled in this row |

## 2. Disagreement one: is brain-like actually the goal (this one matters most)

The cognitive-science camp's conclusion is the opposite of your framing:

- The human brain by itself cannot do the "thirteen constraints consistent across a hundred rounds" you
  want — working memory holds about 4 items (Cowan 2001); people manage it only through **writing things
  down externally** (Clark & Chalmers 1998). So what you want was never "the brain"; it is "brain + scaffold".
- The brain's failure modes happen to be exactly the ones you complain about in AI: **stopping at the first
  plausible explanation** (Simon 1956) and confirmation bias (Wason 1960). Copying the brain copies the bugs
  along with it.

But it also gives the only three things in "brain-like" worth keeping, and these three land squarely in
engineering:

| Mechanism | Biological anchor | Engineering meaning |
|---|---|---|
| Sparse competition (k-WTA) | CA3 pattern completion, basal-ganglia gating | H must be a **mutually inhibiting set**, not a string of synonymous restatements |
| Prediction error updates only where it fails | predictive processing (Rao & Ballard 1999) | every operator must emit "what I predicted", otherwise it cannot be falsified |
| Offline replay + a single gate | hippocampal replay (Wilson & McNaughton 1994) | the ledger is not a log, it is the scheduler's training set |

**My verdict**: rewrite the goal from "brain-like" to "**an auditable scaffold**" — keep the three above,
drop "brain-like" as a criterion. This does not weaken your idea; it rescues it from rhetoric: what you
actually want was never "the same as a human", it is "not treating my samples as objects".

## 3. Disagreement two: which operator first, and where negative examples come from

Engineering camp: do **induction** first (it alone produces the h that the other three consume); with just
two, do "induction + deduction", sharing `verify(h,x)`.
ML camp: don't rush to choose an operator; first settle **where E⁻ comes from** — line 1178 of 1.md writes
about counterexamples, and the whole text contains not one real counterexample.
Skeptic camp: neither is enough yet; pre-register 50 cases first, otherwise the evaluation is undecidable.

The three do not contradict each other; it is a question of **order**. My judgment: the ML camp takes
priority. The reason is decidable: with only positive examples the concept is not identifiable (1.md itself
cites Gold 1967 and line 1207, "infinitely many definitions"), so doing induction first would produce a
module that **can never be judged wrong** — exactly the number-one failure mode the skeptic camp predicts.

Corollary: among the 15 mechanisms of 1.md, what is missing is not more ② (hypothesis generation) but ④
(**evidence gathering**) — counterexample search, active learning, experiment design. This class occupies
only 3 items in §7, and all are written as "worth borrowing", but it is the only entrance that turns
non-identifiability into identifiability. See [12](12-operator-typing.md) §1.

## 4. Disagreement three: at which layer the unification sits

- Logic camp: deep unification (one formal system + reflection). It can do typed I/O, evidence terms,
  failure = no derivation; it cannot do self-discovery of rule sets.
- Engineering camp: shallow unification (one data structure + one scoring function + one loop).
- ML camp: it cannot be unified mathematically; only the interface can.
- Skeptic camp: line 2005 of 1.md unifies only the signature, and line 2034 itself admits the semantics are
  not unified — that is an API, not a theory.

Three of the four votes say "only shallow unification is possible", and all point to the same ceiling:
**changing representation is undecidable** (line 714 of 1.md itself admits Kolmogorov uncomputability). My
judgment: use shallow unification first, but **must** keep the two columns `warrant` and `falsifier`. Without
these two columns, shallow unification degenerates into "a JSON schema + a pile of prompts", which is the
reskinning the skeptic camp names.

## 5. Three points on which the five reports unexpectedly converge

1. **Operators must be classified by "which slice of state they read and write", not by name.** The logic
   camp gave the signature, the engineering camp gave the `read/write` landing points, the cognitive camp
   gave "generation and selection must be separated". Three independent paths point to the same thing: see
   [12](12-operator-typing.md).
2. **Generation and selection must be separated.** Line 1019 of 1.md says so; the engineering camp turns it
   into two functions; the cognitive camp lands it on the basal-ganglia gate.
3. **Bookkeeping can only be append-only.** The engineering camp gives the anti-pattern `current_best.json`;
   the cognitive camp gives the reason (replay needs the full trajectory); the skeptic camp gives the
   criterion (if the agreement rate on re-review two weeks later is <70%, this block is wrong).

## 6. Three things I would change

1. **Add ④ evidence-gathering operators, and treat "negative examples" as a first-order problem.** It is not
   about adding more generators; it is about being able, first, to produce evidence for ¬h.
2. **Drop "brain-like" as the goal; change it to "an auditable scaffold".** Keep the three mechanisms:
   competition / prediction error / replay.
3. **Add a frozen benchmark.** The 50-case pre-registration the skeptic camp wants and the A/B/C/D four
   conditions of the ML camp can be combined into one experiment: a synthetic rule set (20 rules, each with
   4 positives and 4 negatives) → four conditions → see whether the B−A gain disappears under condition D.
   Pure CPU, runnable, refutable.

## 7. A checkable finding: 1.md carries retrieval residue

`1.md` contains 11 instances of `【turn…searchN】`, 9 isolated lines of `+1`, 14 image-proxy links with
`purpose=inline`, and L1307 even spells the model name as "Jev" (as the source speculates). These are
scraping markers, not body text. If this document is to be shown to others, clean them out first; if it is
only for your own reading, ignore them.

## 8. Cross-disciplinary fill-in (that agent timed out mid-way; I filled it in myself)

Spread out the standard paradigms of six disciplines, and they do not give a sixth type of operator; rather,
**each demonstrates one instance of the same class of operator**:

| Discipline | Paradigm | Which class of 12.md it lands in | Failure criterion |
|---|---|---|---|
| Physics | dimensional analysis, limiting cases, symmetry | ③ representation rewriting (shrink the hypothesis space with invariants) | dimensions do not match / the extreme case diverges |
| Mathematics | duality, invariants, counterexample construction | ③ + ④ | the dual problem does not get simpler / the counterexample is unreachable |
| Control | observability, feedback stability | ⑤ control (can the state be inferred back from the output) | unobservable / gain too high, oscillates |
| Biology | comparative method, counterfactuals | ② analogy + ④ evidence gathering | common ancestry not controlled (confounding) |
| Economics | comparative statics, identification strategies | ④ evidence gathering (what variation identifies causality) | underidentified / the instrument is invalid |
| Language | structural mapping, minimal pairs | ② analogy + ④ | the mapping breaks a constraint / cannot differ in exactly one feature |

**What is lost in the merge**: what is really valuable in each discipline is not the name of the paradigm,
but its **cheap test procedure** — the physicist's dimensional check, the linguist's minimal pair, the
economist's identification strategy: all machines that "produce a counterexample within a few minutes".
Merging into 5 classes squeezes these procedures into the single phrase "④ evidence gathering", and thereby
loses "how to cheaply produce a counterexample".
This is exactly the link that 1.md lacks most (see §3). So the fill-in conclusion: **do not merge test
procedures; merge only the types of operators.**

**Where the original example lands**: the thirteen preferences a–m → wanting to find their common S. In
cross-disciplinary terms this has a standard name: the **construct validity** of psychometrics — are the
thirteen indicators one latent factor?
Hence it becomes a decidable problem: have users rate several systems/designs on 13 dimensions, and run a
factor analysis or a single-factor model comparison; if one factor explains most of the variance, "S" exists
and can be written as factor loadings; reverse-inferring the unlisted e, f, g, h from the high-loading
dimensions is "guessing unmeasured indicators from the factor". This road is far smaller than "a
meta-reasoning engine", but **it can run today**.
