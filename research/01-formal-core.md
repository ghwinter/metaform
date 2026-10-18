# 01 Formal core: definitions, theorems, Lean plan

v0.1 · 2026-10-19

## 1. Objects

Fix a finite domain `X` (the object set). A vocabulary is a finite predicate set `V ⊆ (X → Bool)`.

**Signature**: compress the predicate set into a single function to avoid dependent types.

```
sig V x  :  X → Bool        --  sig V x v = (v ∈ V) ∧ v x, i.e. false when v ∉ V
Indist V x y  :⟺  sig V x = sig V y
```

**Definable**: `S` is definable under `V` if and only if there is a Boolean function `φ` with `S = {x | φ (sig V x)}`.
This is exactly "the set written out as an arbitrary Boolean combination of V," equivalent to "the extension of an attribute concept / attribute set" in FCA.

**Query**: `Q = (E⁺, E⁻)`; the oracle is a partial function `O : X ⇀ Bool` that, on each call, takes an object and returns whether it belongs to S. The true S is unknown.

**Candidate intension set**: `F_V(Q) = { S : S definable under V, E⁺ ⊆ S, S ∩ E⁻ = ∅ }`.

## 2. Theorems

### T1 (equivalent characterization of definability)

For any `V`, `S`:

```
S definable under V  ⟺  ∀ x y, Indist V x y → (x ∈ S ↔ y ∈ S)
```

That is: the definable sets are exactly the unions of `Indist V` equivalence classes.

*Proof.* ⇐: take `φ s := ∃ x, sig V x = s ∧ x ∈ S` (classical choice, computability not required). If `x ∈ S` then `φ (sig V x)` holds; conversely `φ (sig V x)` yields `y` with `sig V y = sig V x` and `y ∈ S`, and the premise gives `x ∈ S`.
⇒: `x ∈ S ⟺ φ (sig V x) = φ (sig V y) ⟺ y ∈ S`. ∎

*Lean difficulty*: low, about 60–80 lines. Requires `Classical`; no assumptions beyond the choice axiom.

### T2 (necessity criterion — the main theorem of this paper)

If there exist `x ∈ E⁺` and `y ∈ E⁻` with `Indist V x y`, then `F_V(Q) = ∅`.

*Proof.* Any `S ∈ F_V(Q)` contains `x` and does not contain `y`, contradicting the equivalent characterization in T1. ∎

*Corollary (search is useless)*: within `V`, no matter how much budget is added or which search algorithm is substituted, it is impossible to produce a hypothesis consistent with the samples. **Representation change is a logical necessity, not an optimization option.**

*Lean difficulty*: extremely low, three lines (from T1).

### T3 (growth is monotone)

```
V ⊆ V'  ⟹  Indist V' ⊆ Indist V
```

If `p` separates `x, y` (`p x ≠ p y`) and `p ∉ V`, then `¬ Indist (V ∪ {p}) x y`.

*Proof.* For `v ∈ V`, `v ∈ V'`, hence `v x = v y`; when `v ∉ V` both sides take false. ∎

*Significance*: expanding the vocabulary can only refine it, never coarsen it. So "coarse first, fine later" is a safe search direction, which also explains why attribute exploration can proceed incrementally.

*Lean difficulty*: low.

### T4 (capacity: expressive power is free inside the sample)

Let `m = |{sig V x : x ∈ E}|` (the number of signatures realized on the sample). Then

```
|{ S ∩ E : S definable under V }| ≤ 2^m
```

with equality when every signature cell is either entirely contained in `E⁺` or entirely contained in `E⁻`.

*Proof.* The restriction of `S` to `E` is determined only by "which signatures are selected," giving `2^m` possibilities in total. ∎

*Corollary*: the number of sample-consistent candidate concepts grows exponentially with the number of signatures. If `m` approaches `|E|`, almost any labeling is definable — the consistency test has no binding force on the vocabulary at all. **To generalize, one must have `m ≪ |E|`, that is, there must be a bias class restricting the ways the vocabulary can grow.**

*Lean*:
- weak form `capacity_bound` (two definable sets that attain the same signature set on `E` ⇒ they agree everywhere on `E`);
- **strong form `capacity_bound_strong`** (passed on 2026-10-19): map the restriction `T` to the signature set it attains on `E`;
  this map lands in the power set of the signature set and is injective on the restriction family, so `#R ≤ 2^m`.
  `R` is given as a parameter ("every `T ∈ R` is the intersection of some definable set with `E`"), so the universe `Finset (Finset X)` does not have to be constructed.

### T5 (query lower bound)

Let the candidate intension family `F` satisfy: any two distinct elements differ on `X`. If the algorithm is allowed only membership queries (each asking `x ∈ S?`, answered with 1 bit), then identifying any target in `F` requires at least `⌈log₂ |F|⌉` queries.

*Proof.* `k` queries map `F` to `{0,1}^k`; identification requires this map to be injective, hence `|F| ≤ 2^k`. ∎

*Lean difficulty*: low (of the `Fintype.card_le_of_injective` kind). Easier than expected, so it can be done first.

### T6 (sample monotonicity: more examples do not repair an insufficient vocabulary)

If `F_V(E⁺, E⁻) = ∅`, then for any `E⁺ ⊆ E⁺'` and `E⁻ ⊆ E⁻'` we also have `F_V(E⁺', E⁻') = ∅`.

*Proof.* Any `S ∈ F_V(E⁺', E⁻')` necessarily also satisfies the weaker constraints `E⁺ ⊆ S` and `S ∩ E⁻ = ∅`, contradicting `F_V = ∅`. ∎

*Significance*: this is the formal rebuttal of the intuition "give the model a few more examples." More examples only add constraints; they do not change the available language.
Conversely, as the sample grows from small to large, `F` can only go from nonempty to empty — **more data exposes rather than repairs an insufficient vocabulary**.
For the empirical version see `research/exp/RESULTS.md` (`P(F empty)` rising from 2.5% to 98.7% as the number of examples grows).

*Lean*: `no_consistent_concept_mono` (verified).

### T7 (non-monotonicity of MDL and vocabulary, an existence claim)

There exist `Q` and `V ⊂ V'` such that `L(V') + L(E | V') < L(V) + L(E | V)`, and there also exist counterexamples in the other direction.

*Significance*: the payoff of vocabulary expansion is not monotone, and "more predicates is better" is false. This one only requires constructing counterexamples, not a general theorem.

*Lean difficulty*: low (two small `decide`-level examples), but the value is low too; optional.

## 3. Algorithms obtained directly from the theorems

```
State: V (vocabulary), F (candidate intension set, initially = all V-definable sets)
Loop:
  1. If F = ∅          → by T2, the current vocabulary is proven insufficient; enter vocabulary expansion
  2. If |F| = 1        → identified, stop
  3. Choose query object x* = argmax_x  H(F) − E[H(F | answer to x ∈ S?)]   (expected information gain)
  4. Ask the oracle, filter F by the answer
Vocabulary expansion:
  5. The LLM proposes k new predicates p₁..p_k (natural-language description + executable decision procedure)
  6. For each p, estimate how many pairs of candidates in F it separates, and convert via "questioning cost"
  7. Adopt the one maximizing expected gain / cost, or run a membership query on p directly (does this object satisfy p?)
  8. V ← V ∪ {p}, recompute F, return to the loop
```

Three key points:

1. **The criterion for "the language is insufficient" is `F = ∅`**, not the model's self-assessment. This is the direct engineering of T2, and the only reliable metacognitive signal in the whole system.
2. **Questioning is separated from proposing**: the LLM only proposes predicates (generation), the program decides what to ask (selection), and the oracle decides truth (verification). The three do not share the same source of error.
3. **T4's constraint shows up in step 5**: predicates cannot be generated arbitrarily; they must fall inside a restricted language (Boolean combinations / first-order clauses / DSL fragments), otherwise capacity explodes and the learned S has no generalization meaning.

## 4. Lean formalization plan

### 4.1 Engineering choices (important, saves money)

Two **already compiled** copies of Mathlib exist on this machine (6.5 GB each, commit `56a00585`, Lean `v4.35.0`).
**Do not create a new standalone project** — that would mean downloading 6.5 GB again. Add a library to an existing project instead, and point `METAFORM_LEAN_PROJECT` at it:

```
<existing Lake project>\
  lakefile.toml          # already requires mathlib rev v4.35.0
  ReasearchLean/
  Research/
    QueryDriven.lean     # new
```

`lake env lean Research/QueryDriven.lean` suffices; no `lake update` needed.

**Time cost**: cold start of `import Mathlib` measured at **448 seconds**. So do not iterate interactively line by line: write it once, compile the whole file, look at the errors, then change. Count each iteration as 8–10 minutes.

### 4.2 Skeleton (draft, superseded by the actual file)

> The following is the initial draft; **the type of `sig` is wrong**: a signature is "one Bool per predicate," so the type must be
> `(X → Bool) → Bool`; writing `X → Bool` squeezes all predicates onto the same argument, and `sig V x v` cannot be evaluated.
> Take `research/lean/QueryDriven.lean` as authoritative (compiled, 0 sorry).

```lean
import Mathlib.Data.Finset.Basic
import Mathlib.Data.Set.Finite
import Mathlib.Data.Fintype.Card

namespace QueryDriven

variable {X : Type*} [Fintype X] [DecidableEq X]

/-- Vocabulary: finitely many Bool-valued predicates. -/
def Vocab := Finset (X → Bool)

/-- Signature: false when v ∉ V, avoiding dependent types. -/
def sig (V : Vocab X) (x : X) : X → Bool :=
  fun v => if v ∈ V then v x else false

/-- Indistinguishable under the current vocabulary. -/
def Indist (V : Vocab X) (x y : X) : Prop := sig V x = sig V y

/-- Definable under V: some Boolean function applied to the signature. -/
def Definable (V : Vocab X) (S : Set X) : Prop :=
  ∃ φ : (X → Bool) → Bool, ∀ x, (x ∈ S ↔ φ (sig V x) = true)

/-- T1 -/
theorem definable_iff (V : Vocab X) (S : Set X) :
    Definable V S ↔ ∀ x y, Indist V x y → (x ∈ S ↔ y ∈ S) := by
  sorry

/-- T2: the main theorem. -/
theorem no_consistent_concept
    (V : Vocab X) (Epos Eneg : Set X) (x y : X)
    (hx : x ∈ Epos) (hy : y ∈ Eneg) (h : Indist V x y) :
    ¬ ∃ S, Definable V S ∧ Epos ⊆ S ∧ Disjoint S Eneg := by
  sorry

/-- T3: monotone growth. -/
theorem indist_mono {V V' : Vocab X} (h : V ⊆ V') :
    ∀ x y, Indist V' x y → Indist V x y := by
  sorry

/-- T4: capacity upper bound. -/
theorem capacity_bound (V : Vocab X) (E : Finset X) :
    (Finset.univ.filter (fun T : Finset X => ∃ S, Definable V S ∧
        T = E.filter (· ∈ S))).card ≤ 2 ^ (E.image (sig V)).card := by
  sorry

/-- T5: query lower bound. -/
theorem query_lower_bound (F : Finset (Set X)) (k : ℕ)
    (answers : Set X → Fin k → Bool) (hinj : Function.Injective answers) :
    F.card ≤ 2 ^ k := by
  sorry

end QueryDriven
```

### 4.3 Milestones

| Step | Content | Estimate |
|---|---|---|
| L1 | T3 + T2 (depends on T1's form but can skip the ⇐ direction of `definable_iff`) | 1 compile |
| L2 | T1, full both directions | 2–3 compiles |
| L3 | T4 cardinality counting | 2–3 compiles |
| L4 | T5 injective counting | 1–2 compiles |
| L5 | Executable fragment of the algorithm state machine (`F` filtering and the `F = ∅` test), optional | 2 compiles |

About 300–500 lines in total and 8–10 compiles; at 10 minutes per compile that is **one to two working days** (excluding cold start).

### 4.4 Actual status (measured 2026-10-19 02:55)

**T1, T2, T3, T4 (weak form), and T5 all pass, 0 `sorry`, 0 warnings, compile time 25.5 s.**

```
[check-lean] QueryDriven.lean  exit=0  25.5s
```

How to compile (important, avoid the pitfall):

1. **Do not** create a new Lake project under `research/lean` to run it. A new project makes Lake re-materialize dependencies such as batteries / aesop / Qq / proofwidgets, and these packages **have no precompiled artifacts**, so `lake env lean` immediately reports `unknown module prefix 'Batteries'`. That is exactly how it fails in practice.
2. The workable approach: keep the source in `research/lean/` (it goes into the paper repository), and compile against the environment of an existing Lake project that can load Mathlib (`METAFORM_LEAN_PROJECT`)
   — dependencies there are already compiled, and mathlib goes through `path` to the single global copy.
   `scripts/check-lean.ps1` wraps this copy + compile process:

```powershell
pwsh -File scripts/check-lean.ps1              # about 25 s
pwsh -File scripts/check-lean.ps1 -File X.lean # switch file
```

3. A cold start is 448 s only for a full-package `import Mathlib`; importing just 5 submodules takes about 57 s the first time and 25 s afterwards.

**Deviation from the plan**: T4 formalized only the weak form ("the restriction of a definable set to the sample is determined by the signature set it attains").
The counting version "at most `2 ^ #(E.image (sig V))` of them" requires encoding the restriction set as a subtype of `Finset` and then counting cardinality,
which is grunt work in the Finset API; it is left for L3 and does not affect the conclusions of T2/T5.

## 5. Unresolved, and not pretended to be resolved

1. **Decidability of natural-language predicates**: there is no oracle for whether an LLM-proposed predicate "is elegant" or "is the same concept." The paper deals only with domains where **membership is decidable**.
2. **Noisy oracle**: real users answer wrongly and inconsistently. The T-series theorems all assume a noise-free oracle. Noisy versions (error tolerance > ε) are future work.
3. **Simultaneous growth of multiple predicates**: this paper does only single-step growth (one predicate at a time). Criteria for batch growth (submodularity?) remain unresolved.
4. **Semantic equivalence of vocabulary**: two different predicates may have the same extension (the same partition). Extension equivalence must be quotiented out, otherwise the counting is inflated.
