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

克隆拿到的是 `crates/`、`engine/`、`paper/`、`proofs/` 四类，加上四份文档。
`experiments/` 与 `scripts/` 是工作目录：它们在代码旁边运行、被按路径引用，但不入库，
所以新的克隆只有代码与主张，门禁要自己重建。什么算受跟踪，以 `.gitignore` 顶部的允许清单为准。

总纲 `CHARTER.zh.md`（英文版 `CHARTER.md`）写清问题、十三条带证伪条件的命题、
运行时的六个位置、十条定律与路线。若本 README 与总纲在设计原则上冲突，以总纲为准；若两者与代码或门禁冲突，
以代码与门禁为准。

## 引用规则

受跟踪的文件只能指向仓库里存在的东西。`scripts/check-references.py` 会在这些情况下失败：绝对路径；
指不到任何东西的反引号路径或 markdown 链接（生成的产物、`local.ps1` 引导副本，以及
`scripts/`、`experiments/`、`.dsh/` 三个不入库的工作目录除外）；对未跟踪语料或
对话的裸提及；以及两篇论文里出现的代码标识符、文件名、URL，或没有对应文献条目的引用。论文只通过
文末参考文献接触外部工作，这是常规做法；把定理映射到 Lean 声明的那张表放在
`proofs/mechanization.md`，不再放进论文。

仓库默认拒绝：`.gitignore` 以 `*` 开头，然后放行整个目录（`!/engine/**`）以及少数属于仓库的根文件。
允许目录里新增源文件不需要改规则，新增顶层目录才需要。同一道门会在"受跟踪文件落在所有放行规则之外"
或"放行规则匹配不到任何文件"时失败，所以随手写的笔记或构建产物不可能再意外进入历史。

## 计量式运行与密钥扫描

`scripts/metered-run.ps1` 用 `--json` 跑一次 headless 任务、汇总每步用量并写出指标 JSON；密钥取自
`-KeyEnv` 指定的环境变量或仓库外的 `-KeyFile`，仓库内的密钥文件会被拒绝，来自文件的密钥只放进
子进程环境，只有传 `-Raw` 时才把原始 stdout 留在 tmp。`scripts/check-secrets.py` 扫描受跟踪文件与
tmp 中形似密钥的字符串，支持 `--self-test`；`scripts/check-all.ps1` 把两者作为 `secrets` 门运行。

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
metaform state add   --project . --name p --define all-of:a,b  # 存 p = a AND b 的定义
metaform state vote  --project . --id 0 --yes 3 --no 0
metaform state need  --project .                  # 一步式：直接给状态与建议
metaform apply  --project . --request request.json --response response.json # 校验并采纳被接受的候选，只写一次状态
metaform check-predictions --project .            # 只读：把记录下来的留出预测与当前强标签对账
metaform translate  --input <spec.json|->          # 只读：这次换词是丢失、是换说法、还是真的增产
metaform suggest    --project . [--max-arity 2] [--max 20] [--emit-request [--out <path>]] # 只读：试过哪些布尔候选，以及为何都分不开
metaform explore    --project . [--max-questions 1000] [--laws <path>] [--emit-questions <path>] [--out <path>] # 只读：证据确认的蕴含、带反例的否证、待问的开放问题
metaform answer     --project . --questions <q.json> --answers <a.json> [--laws <path>] # 把专家回答记为定律（define 会采纳谓词），下次 explore 从它们出发
metaform coverage   [--project .] [--format text] [<table.tsv>] # 只读：缺失格、蕴含冲突、等价行、阻断缺口
metaform plan   --project . --pool .metaform/out/pool.txt # 覆盖 + 纯度 + 重问预算
metaform report --project .                         # 生成 .metaform/out/report.md
metaform score  --project . --pool cands.txt --truth truth.txt   # 候选评分：覆盖/P-R/台账
metaform mcp                                       # MCP 服务端（stdio），供 Agent 挂载
metaform metrics verdict <rows.tsv>                # 答出项精度、未知率、selective accuracy、AURC
metaform metrics bets <predictions.tsv>            # 已记录赌注中落在两规则分歧处的比例
metaform metrics readings --vocab <tsv> --readings <tsv>  # 补集覆盖率与轴等价率
metaform ops                                       # 五类算子调度对照：固定顺序 vs 跳过冗余（论文 §4）
```

宿主的注册项为 `serverName` `metaform`、transport `stdio`、command `metaform`、args `["mcp"]`、`cwd` 指向项目根、`toolCallTimeoutMs` 60000、`failOnStartupError` true；模型可见名为 `mcp__metaform__<tool>`。客户端给出任何格式合法的 `protocolVersion` 都原样回显，包括比构建版本更新的修订；只有客户端没给时，才回答 `2055-11-25`。`scripts/install-mcp.ps1` 会把它装到仓库之外的稳定前缀，检查 `--version` 与握手，并把注册项写入每个 profile 的 patch 层、逐个报告动作（`-NoRegister` 只打印注册项，`-DryRun` 只打印计划）；技能文件位于 `.dsh/skills/metaform/SKILL.md`。

台账是**只追加**的：撤销 = 追加一行同 id 的 `retracted`，折叠时后者胜。
本仓库自己也在用：根目录下的 `.metaform/` 就是它的产物（`scripts/migrate-ledger-to-tsv.py` 从旧 JSON 迁来）。

`suggest` 是 `INSUFFICIENT` 状态下只读的证明步骤。它对当前词汇枚举所有 `all_of`（元数 1 到
`--max-arity`）与所有 `any_of`（元数 `--max-arity`），逐个对撞车对打分，按对象掩码去重（保留枚举
顺序中的第一个，操作数写成规范排序），并报告 `tried` 了多少个候选、`deduplicated` 了多少个。
每个撞车对的两个对象签名相同，任何布尔重组在它上面都是常数（定理 11）：所以 `suggestions`
为空，`reason` 说明原因。`next` 指出真正能改变状态的下一步：只要还有撞车等价类包含从未被直接问过
的成员，就再问一个问题（`ask`），验证该成员继承来的标签；否则需要一个当前原子之外的谓词
（`new_atom`）。提问只追加证据，不能移除撞车对（定理 5），所以成员都已被直接问过的类——即使直接
标签互相矛盾——也不再可问。`next` 为 `ask` 时，JSON 还带有 `ask`：对每个仍有未直接提问成员的
撞车等价类各给一个确定性的代表对象（`objects`），`why` 说明原因（`unlabelled_class` 或
`unprobed_member`）。`new_atom` 则完全省略该字段，因为再问也不能修复状态。

加 `--emit-request` 时 `suggest` 不再打印报告。状态为 `INSUFFICIENT` 时，它打印一份可直接填写的
`metaform.propose/1` 请求（或用 `--out <path>` 写入文件），状态保持不变：`protocol`、确定性的
`request_id`（`suggest-` 加 16 位状态哈希）、`objects`（`propose` 读取的对象数）与 `object_ids`、
解析后的 `vocabulary`（每个谓词名及其当前 ids）、`evidence`（推荐 `theta` 下内核看到的 `pos` /
`neg`）、该 `theta`、`colliding_pairs`，以及 `allowed`（外延式 `ids`，或对词汇做 `define`）。该文件
可原样作为 `metaform propose --request <file> --response <file>` 的输入，响应里 `"candidates":[]`
是合法的空回答。其他状态只打印 `{"status":"<STATUS>","request":null}`，不打印请求，因为此时不需要
词汇之外的谓词。

`explore` 是 conexp-clj `explore-attributes` 的只读移植。语境是谓词表（对象 `0..n`、每个谓词的
扩展、每个对象的意图）；专家是证据：推荐 `theta` 下标签已确认的对象。它按固定的 `(基数, mask)`
顺序遍历定律理论的闭集；对前提 `A`，先算定律闭包 `B`，再算证据上的 `B'` = `intent(extent(B))`。
若 `B' == B` 则没有新蕴含；若 `extent(B)` 中有对象缺少 `B' \ B` 的某个属性，该蕴含进入 `refuted`
并附该反例；若没有证据对象匹配前提，则进入 `open`。否则记录定律 `B -> B' \ B`，并以
`by:"evidence"` 报告该蕴含。`--max-questions` 限制检查的前提数，还有剩余工作时追加
`"stopped":"budget"`。只读状态下无法像参考实现那样把反例对象加入语境，所以否证不会缩小后续问题：
只有记录下来的定律（Horn 蕴含）或否证记录会。初始理论从 `.metaform/laws.tsv` 读取（或用
`--laws <path>`；文件不存在即空理论），因此定律或否证已经决定了的前提不会再被检查，也不会再被
提问。输出是一行确定性 JSON
`{"status":...,"questions":...,"budget":...,"implications":[...],"refuted":[...],"open":[...],"closed_sets":...}`；
`status` 与同一 `theta` 下 `state need` 的状态相同，`questions` 是 `open` 的条数，`closed_sets`
是检查过的前提数（后来被定律消掉的前提仍计入），所有名字已排序。`refuted` 条目带 `by`
（本轮证据为 `evidence`，记录下来的为 `expert`）；知道反例对象时还带 `counterexample`。
`--out` 把这一行写入文件而不打印。`--emit-questions <path>` 把开放问题写成
`{"status":...,"questions":[{"id":n,"premise":[...],"conclusion":[...],"text":"Does every object with A also have B?"}]}`
（id 是 `open` 顺序中从 0 开始的下标），并抑制正常的 stdout 行；两个文件都要时再加 `--out`。
状态从不写入，且 `--out` / `--emit-questions` 都不允许指向状态文件或定律文件。

三个命令构成一个闭环：`explore --emit-questions q.json` 提问，专家针对这些 id 写
`{"answers":[...]}`，再由 `metaform answer --project . --questions q.json --answers a.json` 把回答
记入 `.metaform/laws.tsv`（两个命令都可用 `--laws <path>` 指定别处的理论文件）；下次 `explore` 就从
该文件出发。该文件是制表符分隔的，表头为
`source<TAB>kind<TAB>premise<TAB>conclusion<TAB>note`，一行一条记录，名字用逗号分隔；`kind` 取
`law` 或 `refutation`，`source` 取 `evidence`、`expert` 或 `define`。两种记录放在同一个文件里，
两个命令就只有一条理论路径（`--laws`）；`kind` 列保证否证记录不会作为定律进入闭包。`yes` 追加
`expert`/`law` 与问题的前提/结论。`no` 必须带 `{"id":k,"lacks":[names]}`，其中 `k` 是真实对象、
具有全部前提且缺少所列的结论属性，然后追加 `expert`/`refutation`，note 写明 `k`，绝不追加定律。
`explore` 会把该前提视为已决定——不再提问、不再产生蕴含——并在 `refuted` 中以 `by:"expert"`
报告该记录。`define` 通过与 `state add --define` 相同的路径采纳 `name = op:operands`，并追加
`define`/`law`。只有 `define` 回答会写状态，其他回答保持状态不变。重复回答同一份 answers 文件不会
重复写记录（按 kind + source + premise + conclusion 去重）。未知 id、格式错误的回答、并未真正否证
的反例、或已存在的定义名，都会以退出码 2 结束且不写任何文件。

`coverage` 是只读的目标覆盖检查：它与 `explore` 用同一套闭包思想，只是把对象换成了“工作项 × 必需属性”的表。
表是 TSV：表头第一列是 `item`，其后是需求名；每个后续行是一个工作项；单元格为 `yes`、`no`、`?`
或空（空等同 `?`）；空行和以 `#` 开头的行跳过。它先校验整张表，再打印一个 JSON 对象：
`items`、`requirements`、`missing`（每个未知格）、`conflicts`、`equivalences`（yes/no/? 模式完全相同的行）
和 `blocking`。当至少两行对 X 和 Y 都说 `yes` 时，蕴含 `X -> Y` 成立；某行对 X 说 `yes`、对 Y 说 `no` 或
`?`，就是冲突，`implied_by` 与原因会写明提供该模式的行。需求名以 `!` 结尾表示阻断项，输出时去掉
`!`；该列的 `?` 会进入 `blocking`，因为这一行在被填上之前读不出意义，而 `no` 是真实回答、不算缺失。
默认表是 `--project` 下的 `.metaform/coverage.tsv`；位置参数或 `--input` 可以覆盖它。`--format text`
每个问题打印一行 `item: requirement: reason`，便于 shell 循环。命令不写任何文件；非法单元格、重复需求、
未知列或缺少表头都会以退出码 2 结束，stdout 为空。

## LLM 适配器

引擎自己不调用模型，它校验模型返回的东西。`metaform propose` 读取协议 `metaform.propose/1` 的请求文件
与响应文件，报告它接受了什么。候选可以是外延式的（一个名字加它覆盖的对象 id），也可以是定义式的
（对词汇中已有名字做 `all_of` / `any_of`）。校验会拒绝超出对象范围的 id、常量或空扩展、与词汇或先前
候选重名的名字、重复 id，以及协议或请求 id 不符的整份响应；模型也可以拒答，这被记为 `refused` 而不是
重试。校验是纯的：不写文件、不改状态，所以提议无法覆盖证据；采纳由 `apply` 显式完成。
要把被接受的定义持久化下来，用 `metaform state add --project . --name p --define all-of:a,b`
（或 `any-of:a,b`）；`--ids` 仍保持外延式。状态文件会把定义和扩展一起存下，每次加载都按操作数与当前
对象数重算扩展，因此对象集变大后谓词仍然正确。操作数缺失或定义成环都会给出明确错误，而不是静默丢掉
谓词；定义出现之前写下的状态文件照常加载，保持外延式。

`metaform apply --project . --request <request.json> --response <response.json>` 把提议与采纳合成一步：
它复用同一个校验器，按 `state add` 的方式采纳每个被接受的候选（外延式按 ids，定义式存定义、由状态重算
扩展），最后只写一次 `.metaform/state.json`。名字已存在于状态中的候选会被跳过并报告（`name_exists`）；
被校验器拒绝的候选以其原因出现在 `skipped` 中，响应里的其余候选照常采纳。整份响应被拒（协议或请求 id
错误、JSON 损坏）时退出非零且不改文件；定义无法解析时也一样，因为所有采纳都先在副本上完成，最后才写
一次。命令打印一行确定性的 JSON：
`{"applied":n,"skipped":[{"name":...,"reason":...}],"before":{"status":...,"colliding":n},"after":{"status":...,"colliding":n},"request_id":"..."}`。
`before` / `after` 使用与 `suggest`、`state need` 相同的推荐 `theta` 状态与撞车对数，因此这条一步式路径
不会与只读报告不一致。请求和响应文件都只读。

L11 把留出预测做成校验的一部分。带 `object_ids` 的请求同时声明它的留出测试：该列表中不在 `evidence`
里的对象就是*未测量*对象。这样的请求里，候选必须携带 `"predict":{"id":<对象 id>,"label":"pos"|"neg"}`，
否则以 `missing_prediction` 拒绝；被接受时该字段原样回显。只要出现就校验：id 超出 `0..objects` 是
`prediction_out_of_range`，id 已在 `evidence` 中是 `predicted_measured_object`（它不是留出测试），
label 不是 `pos` / `neg` 是 `prediction_bad_label`。不带 `object_ids` 的请求（L11 之前的形状）不声明
未测量对象，所以候选仍可不带该字段，旧判定不变。只有别的规则可能做出不同判断时，预测才算一次真正的
下注；因此候选可以携带它所要竞争的规则：`"rival":{"ids":[...]}"` 或
`"rival":{"define":{"all_of"|"any_of":[...]}}`。rival 按候选自身的规则解析，其扩展由请求词汇重新
算出；预测必须落在这两个扩展判断不同的对象上，否则以 `predict_not_discriminating` 拒绝。带 rival
却没有预测是 `missing_prediction`，因为已经没有什么可比的了。不带 `rival` 的候选行为完全不变、旧判定
不变，于是一条自洽但错误的规则，不能再凭一条邻居规则也会做出的预测通过。

`apply` 把每个被采纳的预测记入 `.metaform/predictions.tsv`：一行表头，之后每个预测一行制表符分隔的
记录（`request_id`、候选名、对象 id、预测标签、候选扩展的逗号分隔 id 列表，以及 rival 的扩展；候选
未指定 rival 时该列为空）。行按 `(request_id, name, id)` 去重，因此重放同一响应不会重复记录，同一
输入写出的字节相同。rival 列在最后，所以在这列出现之前写下的文件仍然加载：五列表头、四列或五列数据
的行都按 rival 为空读取。
`metaform check-predictions --project .` 只读：它用内核同一套推荐 `theta` 强标签，把每条记录与对象的
实际标签比较，打印
`{"checked":n,"correct":n,"wrong":[{"name":...,"id":...,"predicted":...,"actual":...}],"open":n,"discriminating":n}`；
`open` 是对象尚无强标签的预测数，`discriminating` 是已判分、且非空 rival 在预测对象上与候选判断不同
的条数。

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
pwsh -File scripts/check-all.ps1                     # 十一道门（lean/rust/vsa/align/refs/components/claims/secrets/checks/theory/experiments），约 40 s
pwsh -File scripts/check-lean.ps1                    # 只跑 Lean（有 warning 即失败）
pwsh -File scripts/check-rust.ps1                    # 只跑 Rust（通过数必须 == 205）
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
4. **关系型必要性判据。** 树形片段已完成：颜色细化的稳定划分判定两个对象之间是否存在分离模式，
   见证会被最小化成一条可读子句，双相似的一对报出"无分离模式"。未完成：一般合取查询（同态判定，
   一般情形 NP 难），以及把学到的关系接进内核的对象集。
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







































