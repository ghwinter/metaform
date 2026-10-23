# metaform — 查询驱动的表征变更

一个可发表的理论工作 + 可执行参考实现：当目标概念不在当前词汇里时，增加搜索预算必然失败；
系统必须判定"语言不够"，并借外部信号改写语言。

论文：**英文为准** `paper/en/main.tex`（11 页）· 中文翻译 `paper/zh/main.tex`（11 页）。两者都由受跟踪的源文件构建；PDF 是构建产物。
以后改内容只改 `paper/en/main.tex`，再同步 `paper/zh/main.tex`。

## 构建与运行

```powershell
cargo build --workspace --bins
target\debug\metaform.exe crates\metaform\scenarios\verify.demo   # 跑一个场景，不需要项目目录
target\debug\metaform.exe check all                               # 仓库对自己的规则
```

## 结构

| 目录 | 内容 |
|---|---|
| `crates/` | 九个工作区成员，全在 `crates/` 下：度量（`kernel`）、协议（`propose`）、存储、运行时、接口，以及每个提议器一个 crate（`vsa`、`sme`、`ilp`、`components`） |
| `paper/en/`, `paper/zh/` | 双语正文。en 用 `pdflatex` 两遍，zh 用 `xelatex` 两遍（ctex） |
| `proofs/` | Lean 4 形式化：13 条声明承载定理 1–11，不接受未证目标、无警告；`proofs/mechanization.md` 把定理映射到声明 |
| `docs/` | 手册：命令、协议、组件、检查了什么、结果 |
| `experiments/` | Python 实验：合成域、Lean 域、代码域、成本分解、数据总账 `RESULTS.md` |
| `scripts/` | 断言门、对齐与引用检查、Lean/Rust 封装、打包 |

克隆拿到的是 `crates/`、`paper/`、`proofs/`、`docs/` 四类，加上四份文档与构建文件。
`experiments/` 与 `scripts/` 是工作目录：它们在代码旁边运行、被按路径引用，但不入库，
所以新的克隆只有代码与主张，门禁要自己重建。什么算受跟踪，以 `.gitignore` 顶部的允许清单为准。

## 文档

| 页 | 回答什么 |
|---|---|
| `docs/cli.md` | 每个命令、场景脚本语言、每份只读报告的含义 |
| `docs/protocol.md` | `metaform.propose/1` 的请求与响应、留出预测、证伪条件、`apply` 留下的记录 |
| `docs/components.md` | 哪个 crate 提议什么，控制器如何采纳与记录，分层由谁强制 |
| `docs/verification.md` | 两道门、断言清单、引用规则、如何复现每个测量 |
| `docs/results.md` | 二十条可核对的结论、诚实边界、剩余路线 |

总纲 `CHARTER.zh.md`（英文版 `CHARTER.md`）写清问题、十三条带证伪条件的命题、
运行时的六个位置、十条定律与路线。若本 README 与总纲在设计原则上冲突，以总纲为准；若两者与代码或门禁冲突，
以代码与门禁为准。

## 门禁

两道，因为造的是两样东西。`scripts/check-all.ps1` 管程序：测试、实验、以及关于它们的断言。
`scripts/check-paper.ps1` 管论文与形式化：Lean、双语对齐、以及构建出的 PDF 是否旧于它的源。
编译过的形式化与排好版的 PDF 不属于"构建程序"，所以不在程序的门里；每道门逐条断言什么，见
`docs/verification.md`。

## 状态

回路在场景脚本与项目目录两种形态下都跑通，让它成为回路而不是库的四件事——度量、校验、记录、
只读报告——各自有测试、各自有门。还没有的是强到能撞上论文所述失败模式的提议器：它给出的数字
不大但具体，写在 `docs/results.md` 里，没有做"概括"式的美化。

## 许可

代码、证明与门禁采用 MIT OR Apache-2.0（二选一），文本见 `LICENSE-MIT` 与 `LICENSE-APACHE`。
论文采用知识共享署名 4.0 国际许可（CC BY 4.0）。

## 出处

- 问题来自一次很长的研究对话。那份对话是输入而不是交付物，因此不在本仓库跟踪。
- `CHARTER.zh.md`：从那次对话里提炼的立场与禁令；支撑它们的更长的笔记不入库，等引擎稳定后再考虑。
- 36 篇逐篇对照（确认"查询失败 → 改写语言"的空位）也属于那些笔记；论文写出结论与引用。
