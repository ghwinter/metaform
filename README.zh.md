# metaform — 查询驱动的表征变更

一个可发表的理论工作 + 可执行参考实现：当目标概念不在当前词汇里时，增加搜索预算必然失败；
系统必须判定"语言不够"，并借外部信号改写语言。

论文：**英文为准** `paper/en/main.tex`（11 页）· 中文翻译 `paper/zh/main.tex`（11 页）。两者都由受跟踪的源文件构建；PDF 是构建产物。
以后改内容只改 `paper/en/main.tex`，再同步 `paper/zh/main.tex`。

## 结构

| 目录 | 内容 |
|---|---|
| `paper/en/`, `paper/zh/` | 双语正文。en 用 `pdflatex` 两遍，zh 用 `xelatex` 两遍（ctex） |
| `proofs/` | Lean 4 形式化：13 条声明承载定理 1–11，不接受未证目标、无警告；`proofs/mechanization.md` 把定理映射到声明 |
| `experiments/` | Python 实验：合成域、Lean 域、代码域、成本分解、数据总账 `RESULTS.md` |
| `engine/` | Rust 内核 + `metaform` 命令行（`engine/scenarios/` 里有示例脚本） |
| `scripts/` | 断言门、对齐与引用检查、Lean/Rust 封装、打包 |

总纲 `CHARTER.zh.md`（英文版 `CHARTER.md`）写清问题、十三条带证伪条件的命题、
运行时的六个位置、十条定律与路线。若本 README 与总纲在设计原则上冲突，以总纲为准；若两者与代码或门禁冲突，
以代码与门禁为准。

## 引用规则

受跟踪的文件只能指向仓库里存在的东西。`scripts/check-references.py` 会在这些情况下失败：绝对路径；
git 未跟踪的反引号路径或 markdown 链接（生成的产物与 `local.ps1` 引导副本除外）；对未跟踪语料或
对话的裸提及；以及两篇论文里出现的代码标识符、文件名、URL，或没有对应文献条目的引用。论文只通过
文末参考文献接触外部工作，这是常规做法；把定理映射到 Lean 声明的那张表放在
`proofs/mechanization.md`，不再放进论文。

仓库默认拒绝：`.gitignore` 以 `*` 开头，然后放行整个目录（`!/engine/**`）以及少数属于仓库的根文件。
允许目录里新增源文件不需要改规则，新增顶层目录才需要。同一道门会在"受跟踪文件落在所有放行规则之外"
或"放行规则匹配不到任何文件"时失败，所以随手写的笔记或构建产物不可能再意外进入历史。

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

## LLM 适配器

引擎自己不调用模型，它校验模型返回的东西。`metaform propose` 读取协议 `metaform.propose/1` 的请求文件
与响应文件，报告它接受了什么。候选是外延式的：一个名字加它覆盖的对象 id。校验会拒绝超出对象范围的
id、常量或空扩展、与词汇或先前候选重名的名字、重复 id，以及协议或请求 id 不符的整份响应；模型也可以
拒答，这被记为 `refused` 而不是重试。校验是纯的：不写文件、不改状态，所以"加入谓词"仍是独立一步，
提议无法覆盖证据。

在廉价提议的假设下，这个分工才是重点：模型每次给很多候选，引擎支付提问。

## 组件

内核不是全部。算子协议后面有三个组件，各自一个 crate，**都无权改状态**：

| crate | 提议什么 | 方式 |
|---|---|---|
| `crates/vsa` | 与"正负对比向量"相似度高的谓词 | MBAT 绑定/捆绑，`retrieve` |
| `crates/sme` | 通过结构映射推出的谓词 | 结构映射，`analogize` |
| `crates/ilp` | 扩展覆盖正例且不覆盖负例的子句 | 关系学习，`ilp_demo` |
| `crates/runtime` | 不提议任何东西：它是内核 | 协议、校验、控制器、记录、`Operator` |
| `crates/components` | 具体算子与命令行 | `VsaRetriever`、`SmeAnalogizer`、`ProbeLabels`、`metaform-runtime` |

组件提议，内核裁决：

```powershell
target\debug\metaform-runtime.exe retrieve  --state crates/runtime/fixtures/state.txt --out response.json
target\debug\metaform-runtime.exe analogize --state crates/runtime/fixtures/state.txt --systems crates/runtime/fixtures/systems.txt --out response.json
target\debug\metaform.exe propose --request crates/runtime/fixtures/request.json --response response.json
```

采纳由控制器完成，而且会留下记录：

```powershell
target\debug\metaform-runtime.exe run --state crates\runtime\fixtures\state.txt --record runs.tsv
```

它依次询问每个适用的算子，只保留校验器接受的候选，采纳"分开撞车对最多"的那一个（并列时取代价最低），
没有任何候选能分开时停止，并把每一步追加一行——状态指纹、算子、提议、分开对数、是否采纳、代价、
前后状态——所以一串运行事后可以回看。

只有第三行能"接受"，而接受仍不等于采纳：`experiments/proposer_arms.py` 把同一个状态交给三个提议者
（随机、向量检索、语言模型写的候选），由同一个内核裁决并报告结果。

## 仓库对自身的断言

`experiments/claims.tsv` 列出仓库要求读者相信的句子，每条都带一个能否证它的命令。
`scripts/check-claims.py` 负责执行：`PASS` 表示文字仍然成立，`FAIL` 是门禁失败，`NEED` 是还没有
可执行检查的断言——也就是"说了但没验证"的诚实清单。新增一条断言，就在同一个 commit 里补上它的命令。

## 提交信息

遵循 Conventional Commits：类型（`feat`、`fix`、`docs`、`test`、`refactor`、`chore`）、可选 scope、
祈使句摘要。正文说明改了什么、为什么，语气与代码注释一致：不写本机路径，不提未跟踪材料。

## 三件套复现

```powershell
pwsh -File scripts/check-all.ps1                     # 九道门（Lean/Rust/VSA/引用/claim/组件/证伪/双语/数字），约 40 s
pwsh -File scripts/check-lean.ps1                    # 只跑 Lean（有 warning 即失败）
pwsh -File scripts/check-rust.ps1                    # 只跑 Rust（通过数必须 == 71）
pwsh -File scripts/selftest-gates.ps1                # 负向自检：让每套门各红一次
python scripts/align-papers.py                       # 双语结构/数字对齐 + 形式化对应表 ↔ Lean 源
python scripts/check-references.py                   # 不许出现仓库里不存在的路径、文件名或链接
pwsh -File scripts/check-components.ps1                   # 组件、CLI 路径与三提议者实验
python experiments/ledger.py list --all             # 反例台账（可增删改、挂起、带证据与历史）
python experiments/ledger.py show L2               # 看单条：来源、答案、票数、变更轨迹
python experiments/ledger.py corroborate --apply    # 同一对象的弱证据按 noisy-OR 合并
python experiments/ledger.py conflicts              # 方向冲突 + 重问队列（按差距排序，含 json）
pwsh -File scripts/make-bundle.ps1                   # 打提交包（回读核对每个条目）→ dist/submission-<日期>.zip
python experiments/verify_claims.py                 # 论文数字回归（容差 0.06）
python experiments/theory_audit.py --engine target/debug/metaform.exe   # 定理证伪 + 引擎一致性
python experiments/domain_a.py --n 300 --objects 12 --attrs 8 --k 2
python experiments/domain_b.py                      # Lean 当 oracle，约 3 s
python experiments/domain_c.py                      # 真实代码不变量
python experiments/domain_cost.py                   # 成本分解
python experiments/domain_more_data.py              # 定理 5 的经验版
python experiments/greedy_cover.py                  # 贪心覆盖 vs 最优 vs 随机
target\debug\metaform.exe ops                 # 五类算子的调度对照 + 200 次随机自检
python experiments/noisy_oracle.py --attrs 4 --objects 12 --examples 8   # 噪声 oracle
python experiments/proposer.py --case domain_c --emit                    # 看发给模型的请求
python experiments/proposer.py --case domain_c --responses experiments/responses_domain_c.json \
    --pool-out pool.txt --scenario-out case.demo                          # 生成 metaform 场景与候选池
```

脚本从 `METAFORM_PYTHON`（或 PATH 上的 `python`）取解释器。如果 PATH 上的 `python` 是应用商店桩
而不是可用的解释器，每个 shell 设一次：

```powershell
$env:METAFORM_PYTHON = '<python.exe 的完整路径>'
& $env:METAFORM_PYTHON experiments\domain_a.py
```

新克隆的仓库没有本机路径，所以仓库声明它需要什么，而不是带上某一台机器的答案：`environment.json`
写明每项必需能力、提供它的环境变量、最低版本。先检查机器，再让仓库写出它要 source 的文件：

```powershell
pwsh -File scripts/bootstrap.ps1          # 有什么、缺什么、怎么补
pwsh -File scripts/bootstrap.ps1 -Print   # 同样的信息，写给 agent 执行的说明
pwsh -File scripts/bootstrap.ps1 -Write   # 用观测到的值写出 scripts/local.ps1
```

`local.ps1` 是生成物、被 gitignore：它装的是本机事实。版本是另一回事：凡决定结果的东西
（Lean 版本、Mathlib 修订、Rust 工具链、`Cargo.lock`）保持受跟踪，因为两个在这些上不一致的
克隆无法互相校验。

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

18. **廉价的提议去不掉证据地板**：提议成本趋近于零时，成本规律趋于 7.6 个提问轮，所以便宜的是词汇、不是问题；在不足的词汇下花掉的提问无法补回（T5）。候选词汇的组合，代价正好落在它想省下的那种货币上。

19. **廉价的提议买的是词汇，不是提问**：`k` 个候选词汇中恰好一个足够时，随机抽一个成员的基线成功率为 `1/k`（`k=5` 时 0.250），组合为 0.955，代价是每淘汰一个错误候选多花 1.53 次提问（`metaform portfolio`，200 次试验）。组合从不错判扩展；它只以冲突失败，冲突从 `k=3` 的 0.025 升到 `k=10` 的 0.110，因此成功率在 `k=3` 达到峰值 0.975，到 `k=10` 回落到 0.890。

20. **审稿策略基本能被它的分类体系表达，只差一处**：改动前的稿子盲给两位评审，附同一份清单。两人在 16 条中有 14 条一致（Cohen's κ = 0.67）；作者自己的标注与两人分别只一致 11 条和 9 条（κ 为负），因此这些标注不构成稳定策略，本文不采用。在两人一致的 14 条上，分类体系（摘要、数字、交叉引用、标题、图注、术语、排版、记号、措辞）仍使状态耗尽：措辞类里"proved"（被拒——探针是演绎反驳）与时态/语域（被采纳）分开，给出撞车对 (11,13) 与 (12,13)。再加一个谓词"断言强度"就足以识别规则。`engine/scenarios/review.demo` 复现这两个状态。
## 诚实边界

- 核心数学是 FCA/ILP 的 folklore（"可定义 = 等价类之并"）。贡献是把它当**必要性判据**用，
  并补上多算子的定型（论文 §4：单调、幂等、唯一减小、重组不破撞），触发、基准与负结果跟着来。
- 实验都在合成域与一个小代码域上，**没有真 LLM 提议器**在环里；论文据此声明范围，不吹。
- T4 的计数形式 `2^m` 已形式化为 `capacity_bound_strong`，弱形式保留作引理。

## 剩余路线

这次发布停在一条已校验的边界上，而不是一个运行时。按顺序还剩：

1. **把 Lean 放进仓库。** 形式化检查借用的是一个路径属于本机的 Lake 工程，所以克隆之后能读证明，
   却不能重编译。把 lakefile 与工具链（含 Mathlib 修订）跟踪起来、生成 manifest，让 `check-lean`
   就地跑 `lake env lean`。验收：只需 `lake update` 的克隆也能报出同样的 13 条声明、零警告。
2. **接真实模型。** 提议边界是纯的、带校验的，但到目前为止每个响应都是 mock 造的。适配器不改引擎
   代码，只改填响应的那个文件。验收：同一个回路对着模型跑，记录接受/拒绝数与提问数，并做等预算的
   “直接作答 vs 回路”对照。
3. **给 `decompose` 和 `analogize` 定型。** 现有五类管的是证据与表征；目标分解改的是目标，结构映射
   改的是关系结构，两者对度量都没有被陈述的效应，调度器无法推理。验收：每个算子一条命题、一个
   Rust 测试，并在不破坏定理 8–11 的前提下扩展算子表。
4. **关系型必要性判据。** 把撞车对从一元签名推广到类型化关系，结构图才成为计算。验收：一个"一元
   谓词够用、关系必需"的域，引擎报出"无分离模式"。
5. **不会固化的记忆。** ledger 记的是证据，不是策略。只收 ledger 已验证条目的算子经验表，以及用
   "跨任务词汇增长"表达的抽象库，是元推理运行时需要的两块。验收：两个任务族、每个任务的提议数
   可测下降，且没有未经证实的策略被采纳。

## 许可

代码、证明与门禁采用 MIT OR Apache-2.0（二选一），文本见 `LICENSE-MIT` 与 `LICENSE-APACHE`。
论文采用知识共享署名 4.0 国际许可（CC BY 4.0）。

## 出处

- 问题来自一次很长的研究对话。那份对话是输入而不是交付物，因此不在本仓库跟踪。
- `CHARTER.zh.md`：从那次对话里提炼的立场与禁令；支撑它们的更长的笔记不入库，等引擎稳定后再考虑。
- 36 篇逐篇对照（确认"查询失败 → 改写语言"的空位）也属于那些笔记；论文写出结论与引用。







































