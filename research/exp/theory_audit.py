"""
证伪器：用暴力枚举去撞论文里的定理，撞不倒才算没撒谎。

与 verify_claims.py 不同——那里校验"实验数字"，这里校验"定理陈述"本身：
把 n 个对象、m 个谓词的随机形式背景穷举一遍，看有没有反例。

检查项：
  T1  可定义 ⇔ 是等价类的并（用所有布尔函数暴力枚举）
  T2  正反例撞在同一类 ⇔ 候选集为空
  T5  候选集为空后，追加例子仍为空（Lean 文件里编号是 T6）
  T4  可定义集合在样本上的不同限制数 ≤ 2^m
  T7  每个正反例对都被某个谓词分开 ⇔ 没有类同时含正反例
  cover  贪心选出的谓词合起来确实分开所有正反例对（工具构造的正确性）
  ops    五类算子的调度自检：跳过冗余不改变 Φ，省钱数 = 冗余步代价；早停错、探测对（`metaform ops`）

运行：python theory_audit.py [--trials 300]
"""
from __future__ import annotations

import argparse
import itertools
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path


def signatures(preds: list[list[bool]], n: int) -> list[tuple]:
    return [tuple(p[x] for p in preds) for x in range(n)]


def classes_of(sigs: list[tuple]) -> list[list[int]]:
    groups: dict[tuple, list[int]] = {}
    for x, s in enumerate(sigs):
        groups.setdefault(s, []).append(x)
    return list(groups.values())


def definable_by_enumeration(S: list[bool], sigs: list[tuple]) -> bool:
    """枚举所有布尔函数 φ: sig → {0,1}，看是否存在一个与 S 完全一致。"""
    vals = sorted(set(sigs))
    for bits in itertools.product([0, 1], repeat=len(vals)):
        phi = dict(zip(vals, bits))
        if all(phi[sigs[x]] == S[x] for x in range(len(S))):
            return True
    return False


def is_union_of_classes(S: list[bool], cls: list[list[int]]) -> bool:
    return all(len({S[x] for x in c}) == 1 for c in cls)


def candidates(cls: list[list[int]], pos: set[int], neg: set[int]) -> list[frozenset[int]]:
    """独立枚举：所有由等价类拼成的集合里，与样本一致的那些。"""
    out = []
    for mask in range(1 << len(cls)):
        chosen = frozenset(x for i, c in enumerate(cls) if mask >> i & 1 for x in c)
        if pos <= chosen and not (chosen & neg):
            out.append(chosen)
    return out


def separate_all(preds: list[list[bool]], pos: set[int], neg: set[int]) -> bool:
    return all(any(p[x] != p[y] for p in preds) for x in pos for y in neg)


def greedy_cover(preds, pos, neg):
    pairs = [(x, y) for x in pos for y in neg]
    uncovered = set(range(len(pairs)))
    chosen = []
    while uncovered:
        best, gain = None, 0
        for i, p in enumerate(preds):
            g = sum(1 for j in uncovered if p[pairs[j][0]] != p[pairs[j][1]])
            if g > gain:
                best, gain = i, g
        if best is None:
            return None
        chosen.append(best)
        uncovered = {j for j in uncovered if preds[best][pairs[j][0]] == preds[best][pairs[j][1]]}
    return chosen


def engine_status(engine: str, preds: list[list[bool]], pos: set[int], neg: set[int]) -> str:
    """把随机背景写成场景文件，让引擎自己判一次，返回它的 status。"""
    n = len(preds[0]) if preds else 0
    lines = [f"objects {n}"]
    for i, p in enumerate(preds):
        ids = " ".join(str(x) for x in range(n) if p[x])
        lines.append(f"add p{i} {ids}".rstrip())
    for x in pos:
        lines.append(f"vote {x} 3 0")
    for x in neg:
        lines.append(f"vote {x} 0 3")
    lines.append("need")
    with tempfile.NamedTemporaryFile("w", suffix=".demo", delete=False, encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
        path = fh.name
    try:
        out = subprocess.run([engine, path], capture_output=True, text=True, encoding="utf-8")
    finally:
        Path(path).unlink(missing_ok=True)
    for line in reversed(out.stdout.splitlines()):
        if line.startswith("{"):
            return json.loads(line)["status"]
    return "ERROR"


def ops_audit(engine: str) -> int:
    """跑 `metaform ops`：核对自检、三个场景的代价恒等式、以及探测演示的两条断言。"""
    out = subprocess.run([engine, "ops"], capture_output=True, text=True, encoding="utf-8")
    try:
        doc = json.loads(out.stdout.strip().splitlines()[-1])
    except Exception:
        print("  ! ops 输出不可解析")
        return 1
    bad = 0
    audit = doc.get("audit", {})
    if audit.get("violations", 1) != 0:
        bad += 1
        print(f"  ! ops 自检有违反: {audit}")
    for s in doc.get("scenarios", []):
        if s["fixed_cost"] - s["scheduled_cost"] != s["skipped_cost"]:
            bad += 1
            print(f"  ! 代价恒等式不成立: {s['name']}")

    # 无撞车 ≠ 正确：早停必须是错的，探测之后必须是对的，而且验证版要多付代价。
    probe = doc.get("probe", {})
    if probe.get("early_phi") != 1:
        bad += 1
        print(f"  ! 早停时 Φ 不是 1: {probe}")
    if probe.get("early_correct") is not False:
        bad += 1
        print(f"  ! 早停竟然给出了正确外延（探测演示失效）: {probe}")
    if probe.get("verified_correct") is not True:
        bad += 1
        print(f"  ! 探测之后仍然不正确: {probe}")
    if probe.get("verified_cost", 0) <= probe.get("early_queries", 0):
        bad += 1
        print(f"  ! 验证版没有多付代价: {probe}")
    if probe.get("verified_queries") != 5 or probe.get("verified_cost") != 14:
        bad += 1
        print(f"  ! 验证版的固定数字变了（期望 5 次查询、成本 14）: {probe}")

    # 两原子场景：贪心必须两步成功，固定顺序必须失败。
    compose = doc.get("compose", {})
    if compose.get("greedy") != 2.0 or compose.get("greedy_ok") != 1.0:
        bad += 1
        print(f"  ! 贪心没有两步成功: {compose}")
    if compose.get("fixed_ok") != 0.0:
        bad += 1
        print(f"  ! 固定顺序竟然在预算内成功: {compose}")
    if not compose.get("greedy", 9) < compose.get("random", 0):
        bad += 1
        print(f"  ! 贪心不比随机好: {compose}")

    print(f"ops: {len(doc.get('scenarios', []))} 场景 + {audit.get('trials', 0)} 次自检，违反 {bad}")
    return bad


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--trials", type=int, default=300)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--engine", help="qde 可执行文件；给了就核对其判定与理论一致")
    args = ap.parse_args()

    rng = random.Random(args.seed)
    fails = {k: 0 for k in ["T1", "T2", "T6", "T4", "T7", "cover"]}
    examples = 0

    for _ in range(args.trials):
        n = rng.randint(3, 6)
        m = rng.randint(1, 4)
        preds = [[rng.random() < 0.5 for _ in range(n)] for _ in range(m)]
        S = [rng.random() < 0.5 for _ in range(n)]
        sigs = signatures(preds, n)
        cls = classes_of(sigs)

        # T1
        if definable_by_enumeration(S, sigs) != is_union_of_classes(S, cls):
            fails["T1"] += 1

        # 随机样本
        pos = {i for i in range(n) if S[i] and rng.random() < 0.6}
        neg = {i for i in range(n) if not S[i] and rng.random() < 0.6}
        collision = any(any(x in pos for x in c) and any(x in neg for x in c) for c in cls)
        cands = candidates(cls, pos, neg)

        # T2：撞车 ⇔ 候选集为空（候选集由独立枚举得到，不是同义反复）
        if collision != (len(cands) == 0):
            fails["T2"] += 1

        # T6：候选集空后追加例子仍空
        if not cands:
            pos2 = pos | {i for i in range(n) if S[i] and i not in pos and rng.random() < 0.3}
            neg2 = neg | {i for i in range(n) if not S[i] and i not in neg and rng.random() < 0.3}
            if candidates(cls, pos2, neg2):
                fails["T6"] += 1

        # T4：样本上不同限制数 ≤ 2^(不同签名数)
        sample = sorted(pos | neg)
        if sample:
            restr = set()
            # 枚举所有类并（所有可定义集合），统计其在样本上的限制
            for mask in range(1 << len(cls)):
                chosen = {x for i, c in enumerate(cls) if mask >> i & 1 for x in c}
                restr.add(tuple(x in chosen for x in sample))
            m_sig = len({sigs[x] for x in sample})
            if len(restr) > 2 ** m_sig:
                fails["T4"] += 1

        # T7
        if separate_all(preds, pos, neg) != (not collision):
            fails["T7"] += 1

        # cover：贪心选出的谓词合起来应当分开所有对
        if not collision and pos and neg:
            chosen = greedy_cover(preds, pos, neg)
            if chosen is None or not separate_all([preds[i] for i in chosen], pos, neg):
                fails["cover"] += 1
        examples += 1

    if args.engine:
        rng2 = random.Random(args.seed + 1)
        mism = 0
        for _ in range(25):
            n = rng2.randint(3, 7)
            m = rng2.randint(1, 3)
            preds = [[rng2.random() < 0.5 for _ in range(n)] for _ in range(m)]
            pos, neg = set(), set()
            for x in range(n):
                r = rng2.random()
                if r < 0.3:
                    pos.add(x)
                elif r < 0.6:
                    neg.add(x)
            cls = classes_of(signatures(preds, n))
            collision = any(any(x in pos for x in c) and any(x in neg for x in c) for c in cls)
            st = engine_status(args.engine, preds, pos, neg)
            if (st == "INSUFFICIENT") != collision:
                mism += 1
                print(f"  ! 引擎与理论不一致: status={st} collision={collision}")
        print(f"engine: 25 次随机场景，不一致 {mism}")
        fails["engine"] = mism
        fails["ops"] = ops_audit(args.engine)

    print(f"trials={examples}  seed={args.seed}")
    for k, v in fails.items():
        print(f"  {k:6s} 反例 {v}")
    if any(fails.values()):
        print("\n有定理被撞倒——陈述或实现有问题")
        sys.exit(1)
    print(f"\n{len(fails)} 项检查全部零反例")


if __name__ == "__main__":
    main()


