# metaform — 查询驱动的表征变更

一个可发表的理论工作 + 可执行参考实现：当目标概念不在当前词汇里时，增加搜索预算必然失败；
系统必须判定"语言不够"，并借外部信号改写语言。

论文：**英文为准** [paper/en/main.pdf](paper/en/main.pdf)（11 页）· 中文翻译 [paper/zh/main.pdf](paper/zh/main.pdf)（11 页）。
以后改内容只改 `paper/en/main.tex`，再同步 `paper/zh/main.tex`。

## 结构

| 目录 | 内容 |
|---|---|
| `paper/en/`, `paper/zh/` | 双语正文。en 用 `pdflatex` 两遍，zh 用 `xelatex` 两遍（ctex） |
| `research/` | 命题（00）、形式核心（01）、论文骨架（02）、文献综合（03）、引擎设计（04）、可借机制（05）、AI 原生假设（06）、JEV 决策层（07）、断言审计（08）、长文摘记（09）、工具架构（10）、统一算子（11）、算子定型（12）、六种读法（13） |
| `research/lean/` | Lean 4 形式化：11 条定理（13 个 Lean 名称：含多算子的 4 条）+ 3 条辅助引理，0 sorry |
| `research/exp/` | Python 实验：合成域、Lean 域、代码域、成本分解、数据总账 `RESULTS.md` |
| `engine/` | Rust 内核 + `metaform` 命令行（`scenarios/` 里有示例脚本） |
| `scripts/` | 资料库下载/校验、PDF 转文本、Lean 编译封装 |
| `.library/` | **15 个主题目录**、约 8.7k 个文件的离线语料，含 Copycat 替代材料与 JEV 决策模型论文 |
| `.local/` | 本机事实（工具链、路径、坑）与下载笔记 |

## `metaform`：把当前场景摆出来

```powershell
cd engine
cargo run --quiet -- scenarios\setcover.demo
```

命令：`objects N` / `add <name> <id...>` / `pos <id...>` / `neg <id...>` / `vote <id> <yes> <no>` /
`ask` / `status` / `explain` / `score <name> <id...>` / `capacity` / `need [theta]` / `batch k` /
`gen [pool.txt]`（从当前词汇生成 metarule 式候选池）/ `cover <pool.txt>`（贪心覆盖全部撞车对）。

`need` 输出机器可读状态（含 `action`: `PROPOSE` / `ASK` / `STOP`）、票差阈值 `theta`（默认取 `theta_rec`）、
由票数估出的 `epsilon_hat`、推荐的 `theta_rec`，以及需要补票的 `pending`。
噪声 oracle 下用它能压掉假触发（见 `RESULTS.md` §3.11：ε=0.1 时 θ=1 只有 25% 成功，θ=3 是 98%）。

### 项目目录与自举

`metaform` 只写一个子目录 `.metaform/`：

```powershell
metaform init   --project .                      # 建 .metaform/{config.toml,ledger.tsv,logs,out}
metaform ledger add     --project . --case 3 --by demo --reason "第一条证据"
metaform ledger list    --project . --all        # TSV，按 id 折叠出最新状态
metaform ledger retract --project . --id L2 --reason "oracle 答错"
metaform ledger cases   --project .              # 已确认且无冲突的对象
metaform ledger conflicts --project .            # 方向冲突：{"conflicts":[...]}
metaform state init  --project . --n 4            # 场景持久化到 .metaform/state.json
metaform state vote  --project . --id 0 --yes 3 --no 0
metaform state need  --project .                  # 一步式：直接给状态与建议
metaform plan   --project . --pool .metaform/out/pool.txt # 覆盖 + 纯度 + 重问预算
metaform report --project .                         # 生成 .metaform/out/report.md
metaform score  --project . --pool cands.txt --truth truth.txt   # 候选评分：覆盖/P-R/台账
metaform mcp                                       # MCP 服务端（stdio），供 Agent 挂载
metaform ops                                       # 五类算子调度对照：固定顺序 vs 跳过冗余（论文 §4）
```

台账是**只追加**的：撤销 = 追加一行同 id 的 `retracted`，折叠时后者胜。
本仓库自己也在用：根目录下的 `.metaform/` 就是它的产物（`scripts/migrate-ledger-to-tsv.py` 从旧 JSON 迁来）。

## 三件套复现

```powershell
pwsh -File scripts/check-all.ps1                     # 五套断言门（Lean/Rust/证伪/双语/数字），约 40 s
pwsh -File scripts/check-lean.ps1                    # 只跑 Lean（有 warning 即失败）
pwsh -File scripts/check-rust.ps1                    # 只跑 Rust（通过数必须 == 63）
pwsh -File scripts/selftest-gates.ps1                # 负向自检：让每套门各红一次
python scripts/align-papers.py                       # 双语结构/数字对齐 + 附录 Lean 表 ↔ 源文件
python research/exp/ledger.py list --all             # 反例台账（可增删改、挂起、带证据与历史）
python research/exp/ledger.py show L2               # 看单条：来源、答案、票数、变更轨迹
python research/exp/ledger.py corroborate --apply    # 同一对象的弱证据按 noisy-OR 合并
python research/exp/ledger.py conflicts              # 方向冲突 + 重问队列（按差距排序，含 json）
pwsh -File scripts/make-bundle.ps1                   # 打提交包（回读核对每个条目）→ dist/submission-<日期>.zip
python research/exp/verify_claims.py                 # 论文数字回归（容差 0.06）
python research/exp/theory_audit.py --engine engine/target/debug/metaform.exe   # 定理证伪 + 引擎一致性
python research/exp/domain_a.py --n 300 --objects 12 --attrs 8 --k 2
python research/exp/domain_b.py                      # Lean 当 oracle，约 3 s
python research/exp/domain_c.py                      # 真实代码不变量
python research/exp/domain_cost.py                   # 成本分解
python research/exp/domain_more_data.py              # 定理 5 的经验版
python research/exp/greedy_cover.py                  # 贪心覆盖 vs 最优 vs 随机
engine\target\debug\metaform.exe ops                 # 五类算子的调度对照 + 200 次随机自检
python research/exp/noisy_oracle.py --attrs 4 --objects 12 --examples 8   # 噪声 oracle
python research/exp/proposer.py --case domain_c --emit                    # 看发给模型的请求
python research/exp/proposer.py --case domain_c --responses research/exp/responses_domain_c.json \
    --pool-out pool.txt --scenario-out case.demo                          # 生成 metaform 场景与候选池
```

脚本从 `METAFORM_PYTHON`（或 PATH 上的 `python`）取解释器。本机的 `python` 是 Microsoft Store
桩，所以每个 shell 设一次：

```powershell
$env:METAFORM_PYTHON = 'C:\path\to\python.exe'
& $env:METAFORM_PYTHON research\exp\domain_a.py
```

## 结论（可核对）

1. **T2（必要性）**：正反例在同一签名类里，则当前词汇内无一致概念。合成域 300/300 复现。
2. **T5（不可修复）**：更多例子不会修复词汇不足，只会暴露它。P(不足) 随例子数 2.5% → 98.7%。
3. **选题是二阶项**：信息增益与"随机问一个还有信息的问题"持平；加先验也只领先 9–13%。
4. **成本由提议质量决定**：总成本跨 45×，几乎全由 `r/q` 决定；停止规则差 ±6%。
   `verify_claims.py` 现在检查三件事：`r/q+7.6` 与四个实测点的相对误差 <4%；论文里不许再出现
   `exactly`；并真的去跑 `metaform ops` 核对 `3.11/3.50/3.60` 与探测的 `1/5/14`，再把 README
   里同一组数字解析出来做第三方对照（论文、README、程序三方一致才算过）。
5. **错谓词零帮助**：在撞车对上取值相同的谓词，不会改变任何签名。
6. **T8–T11（算子定型）**：五类步骤共用"活假设个数"这一个度量。生成只抬高它，闭包幂等，
   只有取证严格降低它，词汇内部的布尔重组打不破撞车；表征的 warrant 要求新原子在词汇外、
   且它承认的候选在新词汇下可定义（否则只是把没表达的东西记成已表达）。覆盖（T7）只作
   调度分数。因此表征变更与取证必须交替。
7. **控制的全部收益 = 冗余步的代价**：`metaform ops` 三个场景里"全部执行 vs 跳过冗余"的差额
   恒等于被跳过步骤的代价；200 次随机调度自检 0 违反。
8. **同预算下"选问题"值约 0.4 次查询**：八个活候选、单位代价时，逐步选择 3.11 次、随机顺序
   3.50 次、固定顺序 3.60 次（`metaform ops` 的 `selection` 字段，200 次）。选择是二阶项，
   不是一阶项——这和论文 §5 的负结果一致。
9. **两个原子时贪心组合仍成立**：需要 2 个原子的场景里，贪心 2.00 个原子、成功率 1.00；
   随机顺序 2.87 / 0.13；固定顺序（整类原子在前）3.00 / 0.00（`compose` 字段，200 次）。
10. **真实域上排序可以替代好提议器**：域 C 的四个候选（三个诱饵 + 一个正确）按覆盖排序后
    1 次提议、成本 28，等于"一次就提对"；文件顺序 4 次、88；随机顺序 2.46 次、57.2
    （`selection_domain_c.py`，随机取 200 个 seed）。
11. **无撞车 ≠ 正确**：需要两个谓词的域 C 场景里，标签在类内继承会让循环以错误外延停机——
    贪心 1 次提议、成本 28、但外延不等于目标。对每个多成员类各探一个没问过的成员后，状态重新
    变成不足，两次提议恢复正确（成本 51，文件顺序 111）；单原子情形只多 3 次查询（28 → 31）。
12. **内核停机现在要求"无继承标签"**：`run_selected` 只在 `Φ=1` **且** `unprobed` 为空时停；
    否则优先排 `Op::Probe`。探测证伪 → 状态退回不足；探测一致 → 未验证集合缩小；没有可用探测
    算子时按 `Φ=1` 停，但 `unprobed` 会把风险暴露给调用方。
13. **这条现在在工具输出里可见，也被门检查**：`metaform ops` 的 `probe` 字段——早停 1 次查询、
    `Φ=1`、外延错；验证版 5 次查询、成本 14、外延对。`theory_audit.py` 断言这两条；把 JSON 改成
    `early_correct: true` 会让 `check-all` 变红（负向自检已进 `selftest-gates.ps1`）。
14. **CLI 也会要求先验证**：`objects 3; add p 0; vote 0 3 0; vote 1 3 0; need` 的输出是
    `"action":"VERIFY"`、`"unverified":[2]`——类 `{1,2}` 的标签是从 1 继承来的，2 没被直接问过。
15. **端到端跑通一次**：`engine/scenarios/verify.demo`（6 个测试文件、判"是否集成测试"）走完
    `IDENTIFIED/VERIFY → INSUFFICIENT/PROPOSE → UNDETERMINED/ASK → IDENTIFIED/STOP`。
    `e2e_verify.py` 把这条序列与 `unverified=[4,5]` 固化进 `check-all`。
    顺带修了 `unprobed()` 顺序不稳的问题（`classes()` 走 `HashMap`）。
16. **报告里能看出探测改变了判定**：`report` 在探测前给 `VERIFY` + `## To verify`（4, 5），
    探测 4 之后给 `INSUFFICIENT` + 撞车 `(0,4)`。`e2e_verify.py` 的项目模式把这条也固化进
    `check-all`（临时目录建项目、写台账、两读报告）。
17. **工具自己记账**：`state vote --record --by probe --reason "..."` 会把这次投票写进
    `ledger.tsv`（编号 `L1`…），`report` 新增 `## Recent evidence` 段落引用它；E2E 已改用这条路径，
    不再手写 `ledger add`。

## 诚实边界

- 核心数学是 FCA/ILP 的 folklore（"可定义 = 等价类之并"）。贡献是把它当**必要性判据**用，
  并补上多算子的定型（论文 §4：单调、幂等、唯一减小、重组不破撞），触发、基准与负结果跟着来。
- 实验都在合成域与一个小代码域上，**没有真 LLM 提议器**在环里；论文据此声明范围，不吹。
- T4 的计数形式 `2^m` 已形式化为 `capacity_bound_strong`，弱形式保留作引理。

## 出处

- `1.md`：原始对话，提出问题。
- `reasoning_engine_research_map.md`：跨学科资料地图（本仓库 36 篇文献即按它下载）。
- `research/03-synthesis.md`：36 篇逐篇对照，确认"查询失败 → 改写语言"的空位。







































