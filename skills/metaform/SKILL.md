---
name: metaform
description: 判定"当前词汇表达不了这组例子"，并给出该补哪个谓词、该问哪个对象。用于概念发现、谓词提议、归纳失败诊断。
---

# metaform

一个会回答三个问题的工具：

1. 现在的例子能不能被当前词汇表达？（不能 ⇒ `INSUFFICIENT`，别再搜了）
2. 缺的是哪几个谓词？（`cover`：覆盖全部正反例对）
3. 该问谁？（冲突对象按等价类合并，给最少询问数）

## 什么时候用它

- 归纳/拟合反复失败，怀疑"词不够"而不是"搜得不够"
- 有一堆候选谓词，要判断哪些真能分开正反例
- 需要把"我为什么这么判断"落成可复查的证据

## CLI

```powershell
metaform init --project .
metaform state init --project . --n 12
metaform state add  --project . --name overlap --ids 3,4,7,10
metaform state vote --project . --id 3 --yes 3 --no 0
metaform state vote --project . --id 4 --yes 0 --no 3 --record --by probe --reason "探测：与 3 撞车"
metaform state need --project .          # {"status":"INSUFFICIENT","action":"PROPOSE",...}
metaform ledger add --project . --case 3 --by overlap --reason "合并发生"
metaform ledger cases --project .        # 已确认且无冲突
metaform ledger conflicts --project .    # 真值未定，需重问
metaform plan --project . --pool pool.txt # 覆盖 + 纯度 + 预算
metaform report --project .              # 生成 out/report.md（人读）
metaform score --project . --pool cands.txt --truth truth.txt  # 评分
metaform ops                              # 五类算子的调度对照（论文 §4）
```

## MCP

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"state_need","arguments":{"project":"."}}}
```

工具：`state_need`、`state_vote`、`plan`、`ledger_cases`、`ledger_conflicts`。
启动：`metaform mcp`（stdio，行分隔 JSON）。

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



