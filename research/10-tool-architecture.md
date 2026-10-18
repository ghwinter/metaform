# 10 工具架构：从 `qde` 到 `metaform`

目标不是"把脚本包一层"，而是：**一个可被任意项目调用的二进制，把归纳过程的结构留在项目里。**

## 1 四个同类仓库，各学什么、各不学什么

读了 [.repo/](../.repo/README.md) 里的四个仓库。

| 仓库 | 做对了什么 | 不学什么 |
|---|---|---|
| `concepts` | 数据模型干净：`Context` / `Concept` / `Lattice` 可 `fromdict/fromjson/tostring`，序列化是一等公民 | 它假定属性集给定，没有"不够用"的判定 |
| `conexp-clj` | **交互协议**：`explore-attributes` + 反例 handler（完整/不完整两类），把"问什么"与"怎么答"分开 | Clojure 生态、GUI 导向，不适合当 Agent 的内核 |
| `metagol` | 极简：一个 11 KB 的 `metagol.pl` 说清元规则 ILP | Prolog 运行时；它的组合闭包正好证明"组合不产生新原子" |
| `popper` | **输入契约**：examples / background knowledge / bias 三份文件，失败即反馈（learning from failures） | SWI-Prolog 依赖，重；它的语言偏置仍是给定的 |

**可吸收的三条**：

1. 像 `concepts` 一样，**状态可序列化**（JSON/TSV 进 `.metaform/`，人可读、diff 可看）。
2. 像 `conexp-clj` 一样，**把"问什么"和"答什么"拆开**——我们的 `need` 已经是这个形状，继续强化。
3. 像 `popper` 一样，**输入是文件、输出是文件**，二进制不假设调用者是人还是 Agent。

**不吸收的**：它们的搜索算法。我们的判据不是"搜得更好"，恰好是"搜不动了该补词"。

## 2 架构

```
metaform（Rust，单个二进制，无运行时依赖）
├── lib:  Explorer / Status / Ledger（纯逻辑，可测）
└── bin:  CLI —— 读脚本、读写项目目录、输出 JSON
```

- **Rust 是唯一内核**。Python 只保留两类：实验（论文数字）与一次性迁移脚本。
- **CLI 不是本体**。本体是库；CLI 只是第一个前端。后续可加 MCP server / skill，
  它们调用同一个库。
- **无外部依赖**（当前零 crate 依赖）。需要结构化存储时再加 `serde`/`rusqlite`，不提前背。

## 3 产物目录 `.metaform/`

任何项目里，`metaform` 只写一个子目录：

```
<project>/.metaform/
├── config.toml       # 项目级设置（θ、oracle 类型、版本）
├── ledger.tsv        # 证据台账：仅追加，按 id 折叠最新状态
├── state.json        # 当前场景（对象/谓词/票数），可重放
├── logs/             # 每次调用的输入输出，便于复盘
└── out/              # 生成的 md / json 报告
```

约定：

1. **只写 `.metaform/`**，不污染项目其他位置。
2. `ledger.tsv` **只追加**：撤销 = 追加一行同 id 的 `retracted`；折叠时后者胜。
   这样既满足"可改"，又不怕并发写坏。
3. 所有输出同时落 `out/` 与 stdout（Agent 读 stdout，人读文件）。

## 4 迁移顺序

| 步 | 内容 | 状态 |
|---|---|---|
| 0 | `qde` 脚本解释器 | 已有 |
| 1 | 包名 `querydriven` → `metaform`；二进制 `metaform` | 本轮 |
| 2 | `init` 建目录；`ledger` 子命令（add/list/retract/cases/conflicts）写 TSV | 完成 |
| 3 | 方向/冲突/已确认语义进入 Rust | 完成 |
| 4 | `state.json` 快照（`metaform state init/add/vote/need/show`） | 完成 |
| 5 | `metaform mcp`（stdio JSON-RPC）+ `skills/metaform/SKILL.md` | 完成 |
| 6 | `plan`（覆盖/纯度/预算）与 `report` 进入 Rust | 完成 |
| 7 | v0.2.0 发布包（exe + SKILL + VERSION） | 完成 |
| 8 | `score` 进入 Rust；v0.3.0 发布包 | 完成 |



## 5 自举

本仓库也是一个普通项目：`metaform init` 在这里生成 `.metaform/`，
之后所有判断（缺什么、该问谁、证据是否够）都从该目录读。
换版本 = 替换二进制；数据不动。

判断"它是否真的好用"的标准只有一条：**我下次思考时会不会主动打开它**。
若不会，说明它还只是论文的附属品，需要继续迭代。





