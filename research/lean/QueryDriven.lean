/-
查询驱动的表征变更 —— 核心形式化
对应笔记：research/01-formal-core.md

T1 可定义性的等价刻画
T2 必要性判据（主定理）
T3 增长单调 / 单谓词可分
T4 容量：可定义集在样本上的限制由取到的签名决定
T5 查询下界
T6 样本变大不会修复词汇不足
T7 覆盖判据
T8 生成单调 / T9 闭包幂等 / T10 证据唯一严格减小 / T11 重组不破撞车

编号沿用本文件；论文里 T5 与 T6 对调（定理 5 = 更多数据不修复，定理 6 = 查询下界），
附录表按 Lean 名称对应，不按编号。

只导入需要的模块，不 import Mathlib，以缩短冷加载时间。
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

/-- 词汇：有限个 Bool 值谓词。 -/
abbrev Vocab (X : Type*) := Finset (X → Bool)

/-- 签名：把"对象在词汇中各谓词上的取值"打包成一个函数。 -/
def sig (V : Vocab X) (x : X) : (X → Bool) → Bool :=
  fun v => if v ∈ V then v x else false

/-- 在当前词汇下不可分辨。 -/
def Indist (V : Vocab X) (x y : X) : Prop := sig V x = sig V y

/-- 签名在词汇内的谓词上取原值。 -/
theorem sig_apply_of_mem {V : Vocab X} {v : X → Bool} (hv : v ∈ V) (x : X) :
    sig V x v = v x := by
  simp [sig, hv]

/-- 签名在词汇外的谓词上恒为 `false`。 -/
theorem sig_apply_of_not_mem {V : Vocab X} {v : X → Bool} (hv : v ∉ V) (x : X) :
    sig V x v = false := by
  simp [sig, hv]

/-- `S` 在 `V` 下可定义：存在布尔函数 `φ`，使 `x ∈ S ↔ φ (sig V x) = true`。 -/
def Definable (V : Vocab X) (S : Set X) : Prop :=
  ∃ φ : ((X → Bool) → Bool) → Bool, ∀ x : X, (x ∈ S ↔ φ (sig V x) = true)

/-! ## T1 可定义性 = 等价类的并 -/

/-- **T1**：`S` 可定义，当且仅当它在每个不可分辨类上取常值。 -/
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

/-! ## T2 必要性判据 -/

/-- **T2（主定理）**：若正例与反例在当前词汇下不可分辨，
则不存在既与样本一致、又在当前词汇下可定义的概念。

推论：在 `V` 内增加搜索预算不可能找到与样本一致的假设。 -/
theorem no_consistent_concept (V : Vocab X) {Epos Eneg : Set X} {x y : X}
    (hx : x ∈ Epos) (hy : y ∈ Eneg) (h : Indist V x y) :
    ¬ ∃ S : Set X, Definable V S ∧ Epos ⊆ S ∧ S ∩ Eneg = ∅ := by
  rintro ⟨S, hdef, hsub, hdis⟩
  have hxS : x ∈ S := hsub hx
  have hyS : y ∈ S := ((definable_iff V S).mp hdef x y h).mp hxS
  have hmem : y ∈ S ∩ Eneg := ⟨hyS, hy⟩
  rw [hdis] at hmem
  exact Set.notMem_empty y hmem

/-! ## T3 词汇增长的单调性 -/

/-- **T3**：词汇变大，不可分辨关系变细（反向包含）。 -/
theorem indist_mono {V V' : Vocab X} (h : V ⊆ V') {x y : X} (hxy : Indist V' x y) :
    Indist V x y := by
  funext v
  by_cases hv : v ∈ V
  · rw [sig_apply_of_mem hv, sig_apply_of_mem hv]
    have hv' : v ∈ V' := h hv
    have hcongr := congrFun hxy v
    rwa [sig_apply_of_mem hv', sig_apply_of_mem hv'] at hcongr
  · rw [sig_apply_of_not_mem hv, sig_apply_of_not_mem hv]

/-- **T3 推论**：若 `p` 在 `x, y` 上取值不同，加入 `p` 后二者不再不可分辨。 -/
theorem indist_insert (V : Vocab X) {p : X → Bool} {x y : X} (hp : p x ≠ p y) :
    ¬ Indist (insert p V) x y := by
  intro h
  have h1 : p x = p y := by
    have hcongr := congrFun h p
    simpa [sig] using hcongr
  exact hp h1

/-! ## T4 容量 -/

/-- **T4（弱形式）**：两个可定义集若在样本 `E` 上取到完全相同的签名集合，
则它们在 `E` 上处处一致。

由此，`E` 上的可定义限制至多 `2 ^ #(E.image (sig V))` 个：
表达容量只由"样本上实现了多少种签名"决定。 -/
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

/-! ## T5 查询下界 -/

/-- **T5**：若 `k` 次成员查询的答案能唯一确定候选内涵族 `F` 中的元素，
则 `#F ≤ 2^k`。 -/
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

/-! ## T6 样本变大不会修复词汇不足 -/

/-- **T6**：词汇不足不会被"给更多例子"修复。

若在样本 `(E⁺, E⁻)` 上已无一致概念，那么在任何更大的样本上也无一致概念。
直觉：更多例子只增加约束，不改变可用语言。 -/
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

/-! ## T7 覆盖判据：逐对可分 ⇒ 无撞车 -/

/-- **T7**：若每个正例/反例对都被词汇里某个谓词分开，则不存在撞车。

这是"提议谓词 = 对撞车对做集合覆盖"的形式化依据：一份提议只要能覆盖
全部 `(x,y)` 对，就足以让状态脱离 `insufficient`。 -/
theorem no_collision_of_separated (V : Vocab X) {Epos Eneg : Set X}
    (h : ∀ x ∈ Epos, ∀ y ∈ Eneg, ∃ p ∈ V, p x ≠ p y) :
    ¬ ∃ x ∈ Epos, ∃ y ∈ Eneg, Indist V x y := by
  rintro ⟨x, hx, y, hy, hxy⟩
  obtain ⟨p, hpV, hne⟩ := h x hx y hy
  have h1 : sig V x p = sig V y p := congrFun hxy p
  rw [sig_apply_of_mem hpV, sig_apply_of_mem hpV] at h1
  exact hne h1

/-! ## T4 强形式：限制的个数至多 `2^m` -/

/-- **T4（计数形式）**：若每个 `T ∈ R` 都是某个 `V`-可定义集与 `E` 的交，则
`#R ≤ 2 ^ #(E.image (sig V))`。

证明：把 `T` 映到它在 `E` 上取到的签名集合，这个映射落在签名集合的幂集里；
由弱形式 `capacity_bound`，它在 `R` 上单射。 -/
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

/-! ## 多算子：状态、势、以及每一类算子能改变什么

状态 `(H, V, E, B)` 里只留下与形式化相关的一半：候选族 `H`、证据 `E=(E⁺,E⁻)`。
`Phi` 是"活着的假设"个数：它把「词汇不足」(`Phi = 0`)、
「未决」(`Phi ≥ 2`)、「已决」(`Phi = 1`) 统一成同一个可数的量。
下面四条定理说：生成只让它变大，闭包是幂等的，证据是唯一能严格减小它的步骤，
而词汇内部的布尔重组打不破撞车。 -/

/-- 候选内涵族：有限个候选集合。 -/
abbrev Hyps (X : Type*) := Finset (Finset X)

/-- 与证据一致：正例全包含，反例全排除。 -/
def Consistent (S Ep En : Finset X) : Prop := Ep ⊆ S ∧ S ∩ En = ∅

/-- 活着的假设：与当前证据一致的候选。 -/
noncomputable def live (H : Hyps X) (Ep En : Finset X) : Hyps X := by
  classical
  exact H.filter (fun S => Consistent S Ep En)

/-- 势：活着的假设个数。 -/
noncomputable def Phi (H : Hyps X) (Ep En : Finset X) : ℕ := (live H Ep En).card

variable {Ep En : Finset X}

/-! ### T8 生成单调：加候选不会让状态更确定 -/

/-- **T8**：候选族变大，势不减。归纳、溯因、类比都只写在 `H` 上，因此都不减小势。 -/
theorem phi_mono {H H' : Hyps X} (h : H ⊆ H') :
    Phi H Ep En ≤ Phi H' Ep En := by
  unfold Phi
  apply Finset.card_le_card
  intro S hS
  simp only [live, Finset.mem_filter] at hS ⊢
  exact ⟨h hS.1, hS.2⟩

/-! ### T9 闭包幂等：在同一份证据上再检查一遍什么也不删 -/

/-- **T9**：把候选族过滤到与证据一致之后，再过滤一次不改变势。 -/
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

/-! ### T10 证据是唯一严格减小的步骤 -/

/-- **T10**：若两个活假设在 `x` 上取值不同，那么问 `x` 并得到"属于"之后，
势严格下降。这是"取证"这一类算子唯一能推进状态的地方。 -/
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

/-! ### T11 词汇内部的布尔重组打不破撞车 -/

/-- **T11（a）**：由当前词汇布尔组合出来的谓词，在不可分辨的两个对象上取值相同。 -/
theorem recomb_cannot_separate {V : Vocab X} {x y : X} (hxy : Indist V x y)
    (p : X → Bool) (φ : ((X → Bool) → Bool) → Bool) (hp : ∀ z, p z = φ (sig V z)) :
    p x = p y := by
  rw [hp x, hp y, hxy]

/-- **T11（b）**：把这样的谓词加进词汇，撞车依然存在。 -/
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


