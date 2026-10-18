# 01 形式核心：定义、定理、Lean 计划

v0.1 · 2026-10-19

## 1. 对象

固定有限域 `X`（对象集）。词汇是一个有限谓词集 `V ⊆ (X → Bool)`。

**签名**：把谓词集压成一个函数，避免依赖类型。

```
sig V x  :  X → Bool        --  sig V x v = (v ∈ V) ∧ v x，即 v ∉ V 时取 false
Indist V x y  :⟺  sig V x = sig V y
```

**可定义**：`S` 在 `V` 下可定义，当且仅当存在布尔函数 `φ` 使 `S = {x | φ (sig V x)}`。
这就是"用 V 的任意布尔组合写出来的集合"，等价于 FCA 里"属性概念 / 属性集合的外延"。

**查询**：`Q = (E⁺, E⁻)`；oracle 是一个偏函数 `O : X ⇀ Bool`，每次调用给一个对象、返回它属于不属于 S。真实 S 未知。

**候选内涵集**：`F_V(Q) = { S : S 在 V 下可定义, E⁺ ⊆ S, S ∩ E⁻ = ∅ }`。

## 2. 定理

### T1（可定义性的等价刻画）

对任意 `V`、`S`：

```
S 在 V 下可定义  ⟺  ∀ x y, Indist V x y → (x ∈ S ↔ y ∈ S)
```

即：可定义集恰好是 `Indist V` 等价类的并。

*证明。* ⇐：取 `φ s := ∃ x, sig V x = s ∧ x ∈ S`（经典选择，不要求可计算）。若 `x ∈ S` 则 `φ (sig V x)` 成立；反之 `φ (sig V x)` 给出 `y` 使 `sig V y = sig V x` 且 `y ∈ S`，由前提得 `x ∈ S`。
⇒：`x ∈ S ⟺ φ (sig V x) = φ (sig V y) ⟺ y ∈ S`。∎

*Lean 难度*：低，约 60–80 行。需要 `Classical`，不需要 choice 公理之外的额外假设。

### T2（必要性判据 —— 本文的主定理）

若存在 `x ∈ E⁺`、`y ∈ E⁻` 使 `Indist V x y`，则 `F_V(Q) = ∅`。

*证明。* 任一 `S ∈ F_V(Q)` 含 `x`、不含 `y`，与 T1 的等价刻画矛盾。∎

*推论（搜索无用）*：在 `V` 内无论增加多少预算、换多少搜索算法，都不可能产出与样本一致的假设。**表征变更是逻辑必然，不是优化选项。**

*Lean 难度*：极低，三行（由 T1）。

### T3（增长是单调的）

```
V ⊆ V'  ⟹  Indist V' ⊆ Indist V
```

若 `p` 分开 `x, y`（`p x ≠ p y`）且 `p ∉ V`，则 `¬ Indist (V ∪ {p}) x y`。

*证明。* 对 `v ∈ V`，`v ∈ V'`，故 `v x = v y`；`v ∉ V` 时两边都取 false。∎

*意义*：扩词表只能变细，不能变粗。所以"先粗后细"是安全的搜索方向，这也解释了为什么属性探索可以增量进行。

*Lean 难度*：低。

### T4（容量：表达力在样本内免费）

设 `m = |{sig V x : x ∈ E}|`（样本上实现的签名数）。则

```
|{ S ∩ E : S 在 V 下可定义 }| ≤ 2^m
```

且当每个签名单元要么全含于 `E⁺`、要么全含于 `E⁻` 时等号成立。

*证明。* `S` 在 `E` 上的限制只由"哪些签名被选中"决定，共 `2^m` 种。∎

*推论*：样本一致的候选概念数随签名数指数增长。若 `m` 接近 `|E|`，任何标注都几乎可定义——一致性检验对词汇没有任何约束力。**要泛化，必须 `m ≪ |E|`，也就是必须有偏置类限制词汇的可增长方式。**

*Lean*：
- 弱形式 `capacity_bound`（两个可定义集在 `E` 上取到的签名集合相同 ⇒ 在 `E` 上处处一致）；
- **强形式 `capacity_bound_strong`**（已于 2026-10-19 通过）：把限制 `T` 映到它在 `E` 上取到的签名集合，
  该映射落在签名集合的幂集内、且在限制族上单射，于是 `#R ≤ 2^m`。
  `R` 以参数形式给出（"每个 `T ∈ R` 都是某可定义集与 `E` 的交"），这样不必构造 `Finset (Finset X)` 的全集。

### T5（查询下界）

设候选内涵族 `F` 满足：任意两个不同元素在 `X` 上不同。若算法只允许成员查询（每次问 `x ∈ S?`，答 1 bit），则识别 `F` 中任一目标至少需要 `⌈log₂ |F|⌉` 次查询。

*证明。* `k` 次查询把 `F` 映到 `{0,1}^k`；识别要求该映射单射，故 `|F| ≤ 2^k`。∎

*Lean 难度*：低（`Fintype.card_le_of_injective` 类）。比预想的容易，可以先做。

### T6（样本单调性：更多例子不修复词汇不足）

若 `F_V(E⁺, E⁻) = ∅`，则对任意 `E⁺ ⊆ E⁺'`、`E⁻ ⊆ E⁻'` 也有 `F_V(E⁺', E⁻') = ∅`。

*证明。* 任一 `S ∈ F_V(E⁺', E⁻')` 必然也满足更弱的约束 `E⁺ ⊆ S`、`S ∩ E⁻ = ∅`，与 `F_V = ∅` 矛盾。∎

*意义*：这是"再给模型几个例子"这个直觉的正式反驳。更多例子只增加约束，不改变可用语言。
反过来，样本从小到大时 `F` 只可能从非空变空——**更多数据会暴露而不是修复词汇不足**。
经验版见 `research/exp/RESULTS.md`（P(F 为空) 随例子数从 2.5% 升到 98.7%）。

*Lean*：`no_consistent_concept_mono`（已验证）。

### T7（MDL 与词汇的非单调性，存在性命题）

存在 `Q` 与 `V ⊂ V'`，使 `L(V') + L(E | V') < L(V) + L(E | V)`，也存在反向的例子。

*意义*：词汇扩张的收益不是单调的，"更多谓词更好"是错的。这条只用构造反例，不做一般定理。

*Lean 难度*：低（两个 `decide` 级小例），但价值也低，可选。

## 3. 由定理直接得到的算法

```
状态：V（词汇），F（候选内涵集，初始 = 全部 V-可定义集）
循环：
  1. 若 F = ∅          → 由 T2，当前词汇已被证明不够；进入扩词
  2. 若 |F| = 1        → 已识别，停止
  3. 选查询对象 x* = argmax_x  H(F) − E[H(F | 答 x ∈ S?)]   （期望信息增益）
  4. 问 oracle，按答案过滤 F
扩词：
  5. LLM 提议 k 个新谓词 p₁..p_k（自然语言描述 + 可执行判定）
  6. 对每个 p，估计它能把 F 中多少对候选分开，并按"提问成本"折算
  7. 采纳使 期望增益 / 成本 最大者，或直接对 p 做成员查询（这个对象满足 p 吗？）
  8. V ← V ∪ {p}，重算 F，回到循环
```

三个要点：

1. **"语言不够"的判据是 `F = ∅`**，不是模型自我感觉。这是 T2 的直接工程化，也是整个系统唯一可靠的元认知信号。
2. **提问与提议分离**：LLM 只提议谓词（生成），程序决定问什么（选择），oracle 决定真假（验证）。三者不共享同一个错误来源。
3. **T4 的约束体现在第 5 步**：谓词不能任意生成，必须落在一个受限的语言里（布尔组合 / 一阶子句 / DSL 片段），否则容量爆炸，学到的 S 没有泛化意义。

## 4. Lean 形式化计划

### 4.1 工程选择（重要，省钱）

本机已有两处**已编译**的 Mathlib（各 6.5 GB，commit `56a00585`，Lean `v4.35.0`）。
**不要新建独立工程**——那要重新下 6.5 GB。直接在现成工程里加一个库，并把 `METAFORM_LEAN_PROJECT` 指过去：

```
<已有的 Lake 工程>\
  lakefile.toml          # 已 require mathlib rev v4.35.0
  ReasearchLean/
  Research/
    QueryDriven.lean     # 新增
```

`lake env lean Research/QueryDriven.lean` 即可，无需 `lake update`。

**时间成本**：冷启动 `import Mathlib` 实测 **448 秒**。所以不要交互式逐条试错：一次写完、整文件编译、看报错再改。每次迭代按 8–10 分钟算。

### 4.2 骨架（草稿，已被实际文件取代）

> 下面这段是最初草稿，**`sig` 的类型写错了**：签名是"每个谓词上取一个 Bool"，类型必须是
> `(X → Bool) → Bool`；写成 `X → Bool` 会让所有谓词挤在同一个参数上，`sig V x v` 无法求值。
> 以 `research/lean/QueryDriven.lean` 为准（已编译，0 sorry）。

```lean
import Mathlib.Data.Finset.Basic
import Mathlib.Data.Set.Finite
import Mathlib.Data.Fintype.Card

namespace QueryDriven

variable {X : Type*} [Fintype X] [DecidableEq X]

/-- 词汇：有限个 Bool 值谓词。 -/
def Vocab := Finset (X → Bool)

/-- 签名：v ∉ V 时取 false，避免依赖类型。 -/
def sig (V : Vocab X) (x : X) : X → Bool :=
  fun v => if v ∈ V then v x else false

/-- 在当前词汇下不可分辨。 -/
def Indist (V : Vocab X) (x y : X) : Prop := sig V x = sig V y

/-- 在 V 下可定义：某个布尔函数作用于签名。 -/
def Definable (V : Vocab X) (S : Set X) : Prop :=
  ∃ φ : (X → Bool) → Bool, ∀ x, (x ∈ S ↔ φ (sig V x) = true)

/-- T1 -/
theorem definable_iff (V : Vocab X) (S : Set X) :
    Definable V S ↔ ∀ x y, Indist V x y → (x ∈ S ↔ y ∈ S) := by
  sorry

/-- T2：主定理。 -/
theorem no_consistent_concept
    (V : Vocab X) (Epos Eneg : Set X) (x y : X)
    (hx : x ∈ Epos) (hy : y ∈ Eneg) (h : Indist V x y) :
    ¬ ∃ S, Definable V S ∧ Epos ⊆ S ∧ Disjoint S Eneg := by
  sorry

/-- T3：增长单调。 -/
theorem indist_mono {V V' : Vocab X} (h : V ⊆ V') :
    ∀ x y, Indist V' x y → Indist V x y := by
  sorry

/-- T4：容量上界。 -/
theorem capacity_bound (V : Vocab X) (E : Finset X) :
    (Finset.univ.filter (fun T : Finset X => ∃ S, Definable V S ∧
        T = E.filter (· ∈ S))).card ≤ 2 ^ (E.image (sig V)).card := by
  sorry

/-- T5：查询下界。 -/
theorem query_lower_bound (F : Finset (Set X)) (k : ℕ)
    (answers : Set X → Fin k → Bool) (hinj : Function.Injective answers) :
    F.card ≤ 2 ^ k := by
  sorry

end QueryDriven
```

### 4.3 里程碑

| 步 | 内容 | 预计 |
|---|---|---|
| L1 | T3 + T2（依赖 T1 的形式但可先跳过 `definable_iff` 的 ⇐ 方向） | 1 次编译 |
| L2 | T1 完整双向 | 2–3 次编译 |
| L3 | T4 基数计数 | 2–3 次编译 |
| L4 | T5 单射计数 | 1–2 次编译 |
| L5 | 算法状态机的可执行片段（`F` 的过滤与 `F = ∅` 判定），可选 | 2 次编译 |

总计约 300–500 行，8–10 次编译，按每次 10 分钟算是**一到两个工作日**（不含冷启动）。

### 4.4 实际状态（2026-10-19 02:55 实测）

**T1、T2、T3、T4（弱形式）、T5 全部通过，0 个 `sorry`，0 条警告，编译 25.5 s。**

```
[check-lean] QueryDriven.lean  exit=0  25.5s
```

编译方式（重要，别踩坑）：

1. **不能**在 `research/lean` 里新建 Lake 工程跑。新建工程会让 Lake 重新 materialize
   batteries / aesop / Qq / proofwidgets 等依赖，而这些包**没有预编译产物**；
   mathlib 的 `cache get` 钩子在本机失效（见 `.local/machine-facts.md`），
   于是 `lake env lean` 直接报 `unknown module prefix 'Batteries'`。实测就是这么挂的。
2. 可用做法：源码留在 `research/lean/`（进论文仓库），编译借一个能加载 Mathlib 的现成 Lake 工程（`METAFORM_LEAN_PROJECT`）
   的环境——那里依赖已编译好，且 mathlib 走 `path` 指向全局唯一副本。
   `scripts/check-lean.ps1` 把这个复制+编译的过程封好了：

```powershell
pwsh -File scripts/check-lean.ps1              # 25 s 左右
pwsh -File scripts/check-lean.ps1 -File X.lean # 换文件
```

3. 冷启动只是"整包 `import Mathlib`"才 448 s；只导入 5 个子模块时首次约 57 s、之后 25 s。

**与计划的差异**：T4 只形式化了弱形式（"可定义集在样本上的限制由取到的签名集合决定"）。
"至多 `2 ^ #(E.image (sig V))` 个"这个计数版需要把限制集编码成 `Finset` 的子类型再数基数，
属于 Finset API 的苦力活，留到 L3 补，不影响 T2/T5 的结论。

## 5. 还没解决、也不假装解决

1. **自然语言谓词的可判定性**：LLM 提议的谓词"是否优雅""是否是同一个概念"没有 oracle。论文里只处理**成员关系可判定**的域。
2. **噪声 oracle**：真实用户会答错、会不一致。T 系列定理都假设 oracle 无噪。加噪版本（容错 > ε）是后续工作。
3. **多谓词同时增长**：本文只做单步增长（每次一个谓词）。批量增长的准则（子模性？）未解决。
4. **词汇的语义等价**：两个不同谓词可能外延相同（同一划分）。需要商掉外延等价，否则计数会虚高。
