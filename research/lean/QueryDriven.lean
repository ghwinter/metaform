/-
Query-driven representational change — core formalization
Corresponding note: research/01-formal-core.md

T1 Equivalence characterization of definability
T2 Necessity criterion (main theorem)
T3 Growth monotonicity / single-predicate separability
T4 Capacity: the restriction of definable sets to a sample is determined by the signatures realized
T5 Query lower bound
T6 A larger sample does not repair insufficient vocabulary
T7 Covering criterion
T8 Generation monotonicity / T9 Closure idempotence / T10 Evidence is the unique strict decrease / T11 Recombination does not break collisions

Numbering follows this file; in the paper T5 and T6 are swapped (Theorem 5 = more data does not repair, Theorem 6 = query lower bound),
and the appendix table is indexed by Lean name, not by number.

Import only the modules needed, not Mathlib, to shorten cold-load time.
-/
import Mathlib.Data.Finset.Card
import Mathlib.Data.Finset.Powerset
import Mathlib.Data.Fintype.Card
import Mathlib.Data.Fintype.Pi
import Mathlib.Data.Fintype.BigOperators
import Mathlib.Data.Set.Basic

set_option autoImplicit false
set_option linter.unusedSectionVars false

namespace QueryDriven

variable {X : Type*} [Fintype X] [DecidableEq X]

/-- Vocabulary: a finite set of Bool-valued predicates. -/
abbrev Vocab (X : Type*) := Finset (X → Bool)

/-- Signature: packages "the object's values on each predicate in the vocabulary" into a single function. -/
def sig (V : Vocab X) (x : X) : (X → Bool) → Bool :=
  fun v => if v ∈ V then v x else false

/-- Indistinguishable under the current vocabulary. -/
def Indist (V : Vocab X) (x y : X) : Prop := sig V x = sig V y

/-- On predicates inside the vocabulary, the signature takes the original value. -/
theorem sig_apply_of_mem {V : Vocab X} {v : X → Bool} (hv : v ∈ V) (x : X) :
    sig V x v = v x := by
  simp [sig, hv]

/-- On predicates outside the vocabulary, the signature is always `false`. -/
theorem sig_apply_of_not_mem {V : Vocab X} {v : X → Bool} (hv : v ∉ V) (x : X) :
    sig V x v = false := by
  simp [sig, hv]

/-- `S` is definable under `V`: there is a Boolean function `φ` such that `x ∈ S ↔ φ (sig V x) = true`. -/
def Definable (V : Vocab X) (S : Set X) : Prop :=
  ∃ φ : ((X → Bool) → Bool) → Bool, ∀ x : X, (x ∈ S ↔ φ (sig V x) = true)

/-! ## T1 Definability = union of equivalence classes -/

/-- **T1**: `S` is definable if and only if it is constant on every indistinguishability class. -/
theorem definable_iff (V : Vocab X) (S : Set X) :
    Definable V S ↔ ∀ x y : X, Indist V x y → (x ∈ S ↔ y ∈ S) := by
  classical
  constructor
  · rintro ⟨φ, hφ⟩ x y hxy
    rw [hφ x, hφ y, hxy]
  · intro h
    refine ⟨fun s => decide (∃ x : X, sig V x = s ∧ x ∈ S), ?_⟩
    intro x
    constructor
    · intro hx
      exact decide_eq_true ⟨x, rfl, hx⟩
    · intro hx
      obtain ⟨y, hy, hyS⟩ := of_decide_eq_true hx
      exact (h y x hy).mp hyS

/-! ## T2 Necessity criterion -/

/-- **T2 (main theorem)**: if a positive and a negative example are indistinguishable
under the current vocabulary, then no concept exists that is both consistent with the sample
and definable under the current vocabulary.

Corollary: increasing the search budget within `V` cannot find a hypothesis consistent with the sample. -/
theorem no_consistent_concept (V : Vocab X) {Epos Eneg : Set X} {x y : X}
    (hx : x ∈ Epos) (hy : y ∈ Eneg) (h : Indist V x y) :
    ¬ ∃ S : Set X, Definable V S ∧ Epos ⊆ S ∧ S ∩ Eneg = ∅ := by
  rintro ⟨S, hdef, hsub, hdis⟩
  have hxS : x ∈ S := hsub hx
  have hyS : y ∈ S := ((definable_iff V S).mp hdef x y h).mp hxS
  have hmem : y ∈ S ∩ Eneg := ⟨hyS, hy⟩
  rw [hdis] at hmem
  exact Set.notMem_empty y hmem

/-! ## T3 Monotonicity under vocabulary growth -/

/-- **T3**: enlarging the vocabulary refines indistinguishability (reverse inclusion). -/
theorem indist_mono {V V' : Vocab X} (h : V ⊆ V') {x y : X} (hxy : Indist V' x y) :
    Indist V x y := by
  funext v
  by_cases hv : v ∈ V
  · rw [sig_apply_of_mem hv, sig_apply_of_mem hv]
    have hv' : v ∈ V' := h hv
    have hcongr := congrFun hxy v
    rwa [sig_apply_of_mem hv', sig_apply_of_mem hv'] at hcongr
  · rw [sig_apply_of_not_mem hv, sig_apply_of_not_mem hv]

/-- **T3 corollary**: if `p` takes different values on `x, y`, then after adding `p` the two are no longer indistinguishable. -/
theorem indist_insert (V : Vocab X) {p : X → Bool} {x y : X} (hp : p x ≠ p y) :
    ¬ Indist (insert p V) x y := by
  intro h
  have h1 : p x = p y := by
    have hcongr := congrFun h p
    simpa [sig] using hcongr
  exact hp h1

/-! ## T4 Capacity -/

/-- **T4 (weak form)**: if two definable sets realize exactly the same set of signatures on the sample `E`,
then they agree everywhere on `E`.

Hence there are at most `2 ^ #(E.image (sig V))` definable restrictions on `E`:
expressive capacity is determined solely by "how many signatures are realized on the sample". -/
theorem capacity_bound (V : Vocab X) (E : Finset X) (S T : Set X)
    (hS : Definable V S) (hT : Definable V T)
    (h : ∀ s : (X → Bool) → Bool,
      (∃ x ∈ E, sig V x = s ∧ x ∈ S) ↔ (∃ x ∈ E, sig V x = s ∧ x ∈ T)) :
    ∀ x ∈ E, (x ∈ S ↔ x ∈ T) := by
  have key : ∀ (U : Set X), Definable V U → ∀ x ∈ E,
      (x ∈ U ↔ ∃ y ∈ E, sig V y = sig V x ∧ y ∈ U) := by
    intro U hU x hx
    constructor
    · intro hxU
      exact ⟨x, hx, rfl, hxU⟩
    · rintro ⟨y, -, hysig, hyU⟩
      exact ((definable_iff V U).mp hU y x hysig).mp hyU
  intro x hx
  rw [key S hS x hx, key T hT x hx]
  exact h (sig V x)

/-! ## T5 Query lower bound -/

/-- **T5**: if the answers to `k` membership queries uniquely determine an element of the candidate intension family `F`,
then `#F ≤ 2^k`. -/
theorem query_lower_bound {F : Finset (Finset X)} {k : ℕ}
    (ans : Finset X → Fin k → Bool)
    (hinj : ∀ S ∈ F, ∀ S' ∈ F, (∀ i, ans S i = ans S' i) → S = S') :
    F.card ≤ 2 ^ k := by
  have hmain : F.card ≤ Fintype.card (Fin k → Bool) := by
    refine Finset.card_le_card_of_injOn (fun S : Finset X => fun i : Fin k => ans S i) ?_ ?_
    · intro S _
      exact Finset.mem_univ _
    · intro S hS S' hS' heq
      exact hinj S hS S' hS' (fun i => congrFun heq i)
  have hcard : Fintype.card (Fin k → Bool) = 2 ^ k := by
    rw [Fintype.card_pi_const]
    simp [Fintype.card_bool]
  exact hmain.trans_eq hcard

/-! ## T6 Enlarging the sample does not repair insufficient vocabulary -/

/-- **T6**: insufficient vocabulary is not repaired by "giving more examples".

If there is no consistent concept on the sample `(E⁺, E⁻)`, then there is none on any larger sample.
Intuition: more examples only add constraints; they do not change the available language. -/
theorem no_consistent_concept_mono (V : Vocab X) {Epos Eneg Epos' Eneg' : Set X}
    (hp : Epos ⊆ Epos') (hn : Eneg ⊆ Eneg')
    (h : ¬ ∃ S : Set X, Definable V S ∧ Epos ⊆ S ∧ S ∩ Eneg = ∅) :
    ¬ ∃ S : Set X, Definable V S ∧ Epos' ⊆ S ∧ S ∩ Eneg' = ∅ := by
  rintro ⟨S, hdef, hsub, hdis⟩
  refine h ⟨S, hdef, fun x hx => hsub (hp hx), ?_⟩
  ext x
  constructor
  · intro hx
    have : x ∈ S ∩ Eneg' := ⟨hx.1, hn hx.2⟩
    rw [hdis] at this
    exact this
  · intro hx
    exact absurd hx (by simp)

/-! ## T7 Covering criterion: pairwise separation ⇒ no collision -/

/-- **T7**: if every positive/negative pair is separated by some predicate in the vocabulary, then no collision exists.

This is the formal justification for "a proposed predicate = a set cover of the colliding pairs": a proposal that covers
all `(x,y)` pairs suffices to move the state out of `insufficient`. -/
theorem no_collision_of_separated (V : Vocab X) {Epos Eneg : Set X}
    (h : ∀ x ∈ Epos, ∀ y ∈ Eneg, ∃ p ∈ V, p x ≠ p y) :
    ¬ ∃ x ∈ Epos, ∃ y ∈ Eneg, Indist V x y := by
  rintro ⟨x, hx, y, hy, hxy⟩
  obtain ⟨p, hpV, hne⟩ := h x hx y hy
  have h1 : sig V x p = sig V y p := congrFun hxy p
  rw [sig_apply_of_mem hpV, sig_apply_of_mem hpV] at h1
  exact hne h1

/-! ## T4 Strong form: at most `2^m` restrictions -/

/-- **T4 (counting form)**: if every `T ∈ R` is the intersection of some `V`-definable set with `E`, then
`#R ≤ 2 ^ #(E.image (sig V))`.

Proof: map `T` to the set of signatures it realizes on `E`; this map lands in the powerset of the signature set
and, by the weak form `capacity_bound`, it is injective on `R`. -/
theorem capacity_bound_strong (V : Vocab X) (E : Finset X) (R : Finset (Finset X))
    (hR : ∀ T ∈ R, ∃ S : Set X, Definable V S ∧ (↑T : Set X) = (↑E : Set X) ∩ S) :
    R.card ≤ 2 ^ (E.image (sig V)).card := by
  classical
  set f : Finset X → Finset ((X → Bool) → Bool) := fun T => T.image (sig V) with hf
  have hmem : ∀ T ∈ R, f T ∈ (E.image (sig V)).powerset := by
    intro T hT
    obtain ⟨S, _, hTS⟩ := hR T hT
    rw [Finset.mem_powerset]
    intro s hs
    rw [hf, Finset.mem_image] at hs
    obtain ⟨x, hxT, rfl⟩ := hs
    have hxE : x ∈ E := by
      have hx : x ∈ (↑T : Set X) := by simpa using hxT
      rw [hTS] at hx
      exact hx.1
    exact Finset.mem_image_of_mem _ hxE
  have hinj : Set.InjOn f ↑R := by
    intro T1 h1 T2 h2 heq
    simp only [Finset.mem_coe] at h1 h2
    obtain ⟨S1, hS1, hTS1⟩ := hR T1 h1
    obtain ⟨S2, hS2, hTS2⟩ := hR T2 h2
    have memT1 : ∀ x, x ∈ T1 ↔ x ∈ E ∧ x ∈ S1 := by
      intro x
      have : x ∈ (↑T1 : Set X) ↔ x ∈ (↑E : Set X) ∩ S1 := by rw [hTS1]
      simpa using this
    have memT2 : ∀ x, x ∈ T2 ↔ x ∈ E ∧ x ∈ S2 := by
      intro x
      have : x ∈ (↑T2 : Set X) ↔ x ∈ (↑E : Set X) ∩ S2 := by rw [hTS2]
      simpa using this
    have hmem_eq : ∀ s, s ∈ T1.image (sig V) ↔ s ∈ T2.image (sig V) := by
      intro s
      exact Iff.of_eq (by simpa only [hf] using congrArg (fun t => s ∈ t) heq)
    have hiff : ∀ s : (X → Bool) → Bool,
        (∃ x ∈ E, sig V x = s ∧ x ∈ S1) ↔ (∃ x ∈ E, sig V x = s ∧ x ∈ S2) := by
      intro s
      constructor
      · rintro ⟨x, hxE, hxs, hxS⟩
        have hx1 : x ∈ T1 := (memT1 x).mpr ⟨hxE, hxS⟩
        have hmem2 : s ∈ T2.image (sig V) :=
          (hmem_eq s).mp (Finset.mem_image.mpr ⟨x, hx1, hxs⟩)
        obtain ⟨y, hy2, hys⟩ := Finset.mem_image.mp hmem2
        exact ⟨y, ((memT2 y).mp hy2).1, hys, ((memT2 y).mp hy2).2⟩
      · rintro ⟨x, hxE, hxs, hxS⟩
        have hx2 : x ∈ T2 := (memT2 x).mpr ⟨hxE, hxS⟩
        have hmem1 : s ∈ T1.image (sig V) :=
          (hmem_eq s).mpr (Finset.mem_image.mpr ⟨x, hx2, hxs⟩)
        obtain ⟨y, hy1, hys⟩ := Finset.mem_image.mp hmem1
        exact ⟨y, ((memT1 y).mp hy1).1, hys, ((memT1 y).mp hy1).2⟩
    have hEqOn : ∀ x ∈ E, (x ∈ S1 ↔ x ∈ S2) := capacity_bound V E S1 S2 hS1 hS2 hiff
    ext x
    rw [memT1, memT2]
    constructor
    · rintro ⟨hxE, hxS⟩; exact ⟨hxE, (hEqOn x hxE).mp hxS⟩
    · rintro ⟨hxE, hxS⟩; exact ⟨hxE, (hEqOn x hxE).mpr hxS⟩
  calc R.card ≤ ((E.image (sig V)).powerset).card :=
        Finset.card_le_card_of_injOn f hmem hinj
    _ = 2 ^ (E.image (sig V)).card := Finset.card_powerset _

/-! ## Multiple operators: state, potential, and what each operator class can change

Of the state `(H, V, E, B)` only the half relevant to the formalization is kept: the candidate family `H` and the evidence `E=(E⁺,E⁻)`.
`Phi` is the number of "live hypotheses": it unifies "insufficient vocabulary" (`Phi = 0`),
"undecided" (`Phi ≥ 2`), and "decided" (`Phi = 1`) into a single countable quantity.
The four theorems below say: generation only increases it, closure is idempotent, evidence is the only step that strictly decreases it,
and Boolean recombination inside the vocabulary cannot break collisions. -/

/-- Candidate intension family: a finite set of candidate sets. -/
abbrev Hyps (X : Type*) := Finset (Finset X)

/-- Consistent with the evidence: all positives included, all negatives excluded. -/
def Consistent (S Ep En : Finset X) : Prop := Ep ⊆ S ∧ S ∩ En = ∅

/-- Live hypotheses: the candidates consistent with the current evidence. -/
noncomputable def live (H : Hyps X) (Ep En : Finset X) : Hyps X := by
  classical
  exact H.filter (fun S => Consistent S Ep En)

/-- Potential: the number of live hypotheses. -/
noncomputable def Phi (H : Hyps X) (Ep En : Finset X) : ℕ := (live H Ep En).card

variable {Ep En : Finset X}

/-! ### T8 Generation monotonicity: adding candidates does not make the state more determined -/

/-- **T8**: enlarging the candidate family does not decrease the potential. Induction, abduction, and analogy all act only on `H`, so none of them decreases the potential. -/
theorem phi_mono {H H' : Hyps X} (h : H ⊆ H') :
    Phi H Ep En ≤ Phi H' Ep En := by
  unfold Phi
  apply Finset.card_le_card
  intro S hS
  simp only [live, Finset.mem_filter] at hS ⊢
  exact ⟨h hS.1, hS.2⟩

/-! ### T9 Closure idempotence: re-checking against the same evidence deletes nothing -/

/-- **T9**: after filtering the candidate family to those consistent with the evidence, filtering again does not change the potential. -/
theorem phi_closure_idem (H : Hyps X) :
    Phi (live H Ep En) Ep En = Phi H Ep En := by
  unfold Phi
  congr 1
  ext S
  simp only [live, Finset.mem_filter]
  constructor
  · rintro ⟨⟨hS, hc⟩, -⟩
    exact ⟨hS, hc⟩
  · intro hS
    exact ⟨hS, hS.2⟩

/-! ### T10 Evidence is the only strictly decreasing step -/

/-- **T10**: if two live hypotheses differ on `x`, then after querying `x` and getting "belongs", the potential strictly decreases.
This is the only place where the "evidence gathering" operator class can advance the state. -/
theorem phi_evidence_lt {H : Hyps X} {h1 h2 : Finset X} {x : X}
    (_h1H : h1 ∈ live H Ep En) (h2H : h2 ∈ live H Ep En)
    (_hx1 : x ∈ h1) (hx2 : x ∉ h2) :
    Phi H (insert x Ep) En < Phi H Ep En := by
  unfold Phi
  have hsub : live H (insert x Ep) En ⊆ live H Ep En := by
    intro S hS
    simp only [live, Finset.mem_filter] at hS ⊢
    exact ⟨hS.1, fun y hy => hS.2.1 (Finset.mem_insert_of_mem hy), hS.2.2⟩
  have hne : live H (insert x Ep) En ≠ live H Ep En := by
    intro heq
    have h2' : h2 ∈ live H (insert x Ep) En := by rw [heq]; exact h2H
    simp only [live, Finset.mem_filter] at h2'
    exact hx2 (h2'.2.1 (Finset.mem_insert_self x Ep))
  exact Finset.card_lt_card (Finset.ssubset_iff_subset_ne.mpr ⟨hsub, hne⟩)

/-! ### T11 Boolean recombination inside the vocabulary cannot break collisions -/

/-- **T11 (a)**: a predicate built by Boolean combination from the current vocabulary takes the same value on two indistinguishable objects. -/
theorem recomb_cannot_separate {V : Vocab X} {x y : X} (hxy : Indist V x y)
    (p : X → Bool) (φ : ((X → Bool) → Bool) → Bool) (hp : ∀ z, p z = φ (sig V z)) :
    p x = p y := by
  rw [hp x, hp y, hxy]

/-- **T11 (b)**: adding such a predicate to the vocabulary leaves the collision in place. -/
theorem indist_insert_of_eq {V : Vocab X} {p : X → Bool} {x y : X}
    (hxy : Indist V x y) (hp : p x = p y) : Indist (insert p V) x y := by
  funext v
  by_cases hv : v ∈ insert p V
  · rw [sig_apply_of_mem hv, sig_apply_of_mem hv]
    by_cases hvp : v = p
    · rw [hvp]; exact hp
    · have hvV : v ∈ V := by
        rcases Finset.mem_insert.mp hv with h | h
        · exact absurd h hvp
        · exact h
      have h := congrFun hxy v
      rwa [sig_apply_of_mem hvV, sig_apply_of_mem hvV] at h
  · rw [sig_apply_of_not_mem hv, sig_apply_of_not_mem hv]

end QueryDriven


