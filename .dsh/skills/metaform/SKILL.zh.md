---
name: metaform
description: 判定"当前词汇表达不了这组例子"，并给出该补哪个谓词、该问哪个对象。用于概念发现、谓词提议、归纳失败诊断。
---

# metaform

一个会回答三个问题的工具：

1. 现在的例子能不能被当前词汇表达？（不能 ⇒ `INSUFFICIENT`，别再搜了）
2. 缺的是哪几个谓词？（覆盖全部正反例对）
3. 该问谁？（冲突对象按等价类合并，给最少询问数）

## 什么时候用它

- 归纳/拟合反复失败，怀疑"词不够"而不是"搜得不够"
- 有一堆候选谓词，要判断哪些真能分开正反例
- 需要把"我为什么这么判断"落成可复查的证据

## MCP 工具

宿主以 `metaform` 注册这个服务，因此模型可见的工具名是：

| 模型可见工具 | 作用 |
|---|---|
| `mcp__metaform__state_need` | 读 `.metaform/state.json`，返回状态、撞车对与建议标注 |
| `mcp__metaform__state_vote` | 记录一票并写入 `.metaform/state.json` |
| `mcp__metaform__ledger_cases` | 返回台账里已确认且无冲突的对象 |
| `mcp__metaform__ledger_conflicts` | 返回方向冲突的对象 |
| `mcp__metaform__plan` | 覆盖 + 纯度 + 重问预算；候选池路径必须留在项目根内 |
| `mcp__metaform__suggest_emit_request` | 生成可直接填写的 `metaform.propose/1` 请求 |
| `mcp__metaform__apply_response` | 校验提议响应并采纳被接受的候选 |
| `mcp__metaform__check_predictions` | 给 `.metaform/predictions.tsv` 里的留出预测评分 |
| `mcp__metaform__explore_questions` | 跑属性探索并返回开放问题 |
| `mcp__metaform__answer_questions` | 把专家回答追加为定律或否证；重放是安全的 |

所有工具都只返回文本 JSON。宿主用 `metaform mcp` 以 stdio 启动服务（行分隔 JSON-RPC）；
命令行也提供同样的操作。

## 握手

```json
{"jsonrpc":"2.0","id":0,"method":"server/discover","params":{}}
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"example","version":"1"}}}
{"jsonrpc":"2.0","id":2,"method":"notifications/initialized"}
{"jsonrpc":"2.0","id":3,"method":"tools/list"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"state_need","arguments":{"project":"."}}}
```

第一个请求是宿主的旧版探测：带 `-32601` 错误且没有 `result` 成员，说明服务端早于现代发现协议，
宿主会回退到 `initialize`。`protocolVersion` 为 `2024-11-05`、`2025-03-26`、`2025-06-18` 或
`2025-11-25` 时原样回显；其他值一律回答 `2025-11-25`，不把版本钉死。

## 读输出

| 字段 | 含义 |
|---|---|
| `INSUFFICIENT` / `PROPOSE` | 词汇不够，去提议新谓词 |
| `UNDETERMINED` / `ASK` | 还有等价类没标注，去提问 |
| `IDENTIFIED` / `VERIFY` | 已唯一确定，但还有成员没被直接问过：先探测再停 |
| `IDENTIFIED` / `STOP` | 只有一个候选概念，且没有继承标签 |
| `colliding_pairs` | 撞在一起的正反例对 |
| `unverified` | 多成员等价类里没被直接问过的成员（`VERIFY` 时要探它们） |
| `pending` | 票数未达 θ 的对象，先补票 |

## 边界

- 谓词必须有可判定的成员关系（能对单个对象算出 0/1）。
- 它不发明谓词，只判定"必须发明"并给覆盖目标。
- 噪声容忍到每次回答错误率约 0.2；更高需加大 θ 或重问。
