# 形式化对应表

论文中哪条陈述由哪一条 Lean 声明承载。论文只陈述结果并引用证明器；这张表是让两者保持同步的
维护记录。

开发文件是 `proofs/QueryDriven.lean`。它在不接受任何未证目标、不产生警告的前提下编译；
`scripts/check-lean.ps1` 在出现任何警告时失败。

| 结果 | Lean 声明 | 说明 |
|---|---|---|
| 定理 1 | `definable_iff` | 按等价类刻画 |
| 定理 2 | `no_consistent_concept` | 必要性（主结果） |
| 定理 3 | `indist_mono`, `indist_insert` | 增长的单调性 |
| 定理 4 | `capacity_bound_strong` | 计数形式（$2^m$） |
| 定理 5 | `no_consistent_concept_mono` | 更多数据不修复 |
| 定理 6 | `query_lower_bound` | 计数论证，$|\mathcal{F}| \le 2^k$ |
| 定理 7 | `no_collision_of_separated` | 覆盖判据（集合覆盖） |
| 定理 8 | `phi_mono` | 生成不减小活集 |
| 定理 9 | `phi_closure_idem` | 闭包幂等 |
| 定理 10 | `phi_evidence_lt` | 证据严格减小活集 |
| 定理 11 | `recomb_cannot_separate`, `indist_insert_of_eq` | 重组保留撞车 |

未列出的部分，因为它们是支撑引理而非论文陈述：`sig_apply_of_mem` 与 `sig_apply_of_not_mem`
（单调性证明使用），以及 `capacity_bound`（弱计数形式，作为 `capacity_bound_strong` 的引理
保留）。定义 `live` 与 `Phi` 是算子陈述所依据的状态。

`scripts/align-papers.py` 校验这张表：这里点到的每条声明都必须存在于 Lean 文件中，条目数必须
等于两篇论文里声称的数量，且表必须与定理列表同步。
