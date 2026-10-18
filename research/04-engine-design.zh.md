# 04 引擎设计：三层对应物

v0.1 · 2026-10-19

## 0 一句话

理论（Lean）、参考内核（Rust）、实验台（Python）三者共用同一组定义；
引擎本身刻意保持"笨"——只做三件事：判断词汇够不够、问一个代表、记账。

## 1 三层对应

| 论文 | Lean | Rust | Python |
|---|---|---|---|
| 签名 `sig_V(x)` | `QueryDriven.sig` | `Explorer::signature` | `signature()` |
| 等价类 | `Indist` | `Explorer::classes` | `classes()` |
| T2 词汇不足 | `no_consistent_concept` | `Status::Insufficient` | `consistent(...) == []` |
| T4 容量 | `capacity_bound` | `Explorer::capacity` | `full_class` 的基数 |
| T6 更多数据无救 | `no_consistent_concept_mono` | `t6_more_data_never_repairs_insufficiency` | `domain_more_data.py` |
| 扩词表 | `Vocab` 增项 | `Explorer::propose` | `attrs + [a*]` |

Rust 内核：`engine/src/lib.rs`，6 个测试，`cargo test` 全绿（2.7 s）。
Lean：`research/lean/QueryDriven.lean`，6 条定理，0 sorry。
Python 实验：`research/exp/*.py`，`RESULTS.md` 是数据总账。

## 2 状态机（唯一的控制逻辑）

```
        ┌──────────────┐
        │  Insufficient │  ← 存在正反例同类（T2）
        └──────┬───────┘
               │ propose(p)
               ▼
        ┌──────────────┐
        │ Undetermined  │  ← 还有未覆盖的类
        └──────┬───────┘
               │ answer(next_question(), ·)
               ▼
        ┌──────────────┐
        │  Identified   │  ← 每个类都被正例或反例覆盖
        └──────────────┘
```

三条规则，没有别的：

1. `Insufficient` → 必须换词（不能靠再问对象，T6）。
2. `Undetermined` → 问一个未覆盖类的代表（问哪个都行，域 A 已证）。
3. `Identified` → 停，输出外延，不输出故事。

### 噪声下的三条附加规则（`status_with(theta)`）

1. **弱票阻塞**：某类里还有"投过票但未达票差 θ"的成员 → 不算已决（返回 `Undetermined`）。
   否则"一个强标签 + 一个弱反对"会被静默接受，给出错误外延。
2. **先补票再提问**：`next_question_with(theta)` 先返回待补票的对象，再返回未覆盖类的代表。
3. **冲突复核**：撞车先复核两轮再提议谓词（由驱动方做；内核只提供 `strong(theta)` 供判定）。

默认 θ=3（依据 `RESULTS.md` §3.11：ε=0.1 时 θ=1 只有 25% 成功，θ=3 是 98%）。

## 3 引擎明确不做的事

| 不做 | 为什么 |
|---|---|
| 用信息增益选对象 | 与"去冗余随机"持平，收益来自避免冗余而非择优（RESULTS §2） |
| 给谓词打分排序 | 没有可靠信号：谓词的外延要 oracle 才知，语义好坏无人判 |
| 自评答案质量 | 同源自评不可靠（TACL 2024 综述结论） |
| 自动提交"最可能的 S" | 词汇不足时任何提交都是编造；宁可返回 `Insufficient` |

## 4 接口契约（下一步要实现的 trait）

```rust
pub trait Proposer {
    /// 给定样本与当前词汇，提出候选谓词。实现方可以是 LLM、检索、人。
    fn propose(&self, ctx: &Explorer, k: usize) -> Vec<Predicate>;
}

pub trait Oracle {
    /// 成员查询。可返回 `Unknown`，为噪声/拒答留位置。
    fn ask(&mut self, x: usize) -> Option<bool>;
}

pub trait CapacityGuard {
    /// T4：限制词汇的可增长方式，防止"样本内可定义、样本外胡来"。
    fn allow(&self, ctx: &Explorer, p: &Predicate) -> bool;
}
```

职责分离是本文的结构性原则：**提议方不能裁决，裁决方不能提议，选择方不能自评。**

## 5 已知限制（写进论文 §7）

1. 对象数 ≤ 64（`Mask = u64`）；换 `bitvec` 即可，不影响语义。
2. oracle 无噪声；`Oracle::ask` 返回 `Option<bool>` 只是留了位置，没有纠错逻辑。
3. 谓词外延等价的去重没做（两个不同谓词可能同外延，会让容量计数虚高）。
4. >64 类时 `capacity` 会饱和（`checked_shl`）。

## 6 复现

```powershell
cd engine
cargo test                      # 6 项，覆盖 T2/T4/T6
pwsh -File ..\scripts\check-lean.ps1   # 6 条定理
cd ..\research\exp
& $py domain_a.py --n 300 --objects 12 --attrs 8 --k 2
& $py domain_cost.py
```

