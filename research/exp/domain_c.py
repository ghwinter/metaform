"""
域 C：以真实代码行为作 oracle。

隐藏不变量：`merge_intervals` 的输出比输入**短**（发生过合并）。
对象 = 12 个区间列表测试用例；谓词 = 用例的可执行特征；
oracle = 真的跑一遍函数，看输出长度。

初始词汇故意缺 "存在重叠区间" 这个谓词，于是必然触发 T2——
除非提议出正确谓词，否则再问多少个用例都无解（T6）。

运行：python domain_c.py
"""
from __future__ import annotations

import itertools
import random

from engine import Explorer, Status


# ---------- 被测代码 ----------

def merge_intervals(intervals: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """合并重叠区间；假定输入合法（要求 a <= b，且非空）。"""
    if not intervals:
        raise ValueError("empty input")
    for a, b in intervals:
        if a > b:
            raise ValueError(f"bad interval {(a, b)}")
    out: list[list[int]] = []
    for a, b in sorted(intervals):
        if out and a <= out[-1][1]:
            out[-1][1] = max(out[-1][1], b)
        else:
            out.append([a, b])
    return [tuple(x) for x in out]


# ---------- 用例 ----------

CASES: list[list[tuple[int, int]]] = [
    [],                                  # 0  空
    [(1, 2)],                            # 1  单个
    [(1, 2), (3, 4)],                    # 2  不相交
    [(1, 3), (2, 4)],                    # 3  重叠
    [(1, 5), (2, 3)],                    # 4  包含
    [(5, 6), (1, 2)],                    # 5  乱序不相交
    [(1, 2), (2, 3)],                    # 6  端点相接
    [(-3, -1), (-2, 0)],                 # 7  负数重叠
    [(0, 10)],                           # 8  单个大区间
    [(1, 2), (4, 5), (7, 8)],            # 9  三个不相交
    [(1, 4), (2, 5), (3, 6)],            # 10 三个链式重叠
    [(1, 2), (2, 3), (3, 4)],            # 11 链式相接
]


def target(case: list[tuple[int, int]]) -> bool:
    """隐藏目标：输出比输入短（发生合并）。空输入抛异常，不计入。"""
    if not case:
        return False
    return len(merge_intervals(case)) < len(case)


# ---------- 谓词（可执行特征） ----------

def p_empty(c): return len(c) == 0
def p_single(c): return len(c) == 1
def p_has_negative(c): return any(a < 0 or b < 0 for a, b in c)
def p_long(c): return len(c) >= 3
def p_sorted(c): return list(c) == sorted(c)
def p_width_gt3(c): return any(b - a > 3 for a, b in c)


def p_overlap(c):
    """真正需要的那个谓词：存在一对区间重叠或相接。"""
    for (a1, b1), (a2, b2) in itertools.combinations(c, 2):
        if max(a1, a2) <= min(b1, b2):
            return True
    for i, j in itertools.permutations(range(len(c)), 2):
        if c[i][1] == c[j][0]:
            return True
    return False


PREDICATES = {
    "empty": p_empty,
    "single": p_single,
    "has_negative": p_has_negative,
    "long": p_long,
    "sorted": p_sorted,
    "width_gt3": p_width_gt3,
}
NEEDED = ("overlap", p_overlap)   # 初始词汇里没有


# ---------- 诱饵：看起来合理、但分不开撞车那一对的谓词 ----------

def p_all_positive(c): return all(a >= 0 and b >= 0 for a, b in c)
def p_max_len_2(c): return len(c) <= 2
def p_small_gap(c):
    s = sorted(c)
    return any(s[i + 1][0] - s[i][1] <= 1 for i in range(len(s) - 1))


DECOYS = {
    "all_positive": p_all_positive,
    "max_len_2": p_max_len_2,
    "small_gap": p_small_gap,
}


def mask_of(fn) -> int:
    m = 0
    for i, c in enumerate(CASES):
        if fn(c):
            m |= 1 << i
    return m


def main() -> None:
    n = len(CASES)
    target_mask = mask_of(target)
    print(f"cases={n}  P(target)={bin(target_mask).count('1')}/{n}")

    e = Explorer(n)
    for name, fn in PREDICATES.items():
        e.propose(name, mask_of(fn))

    # 找一对"V0 同签名、目标不同"的用例作为初始样本
    pair = None
    for i, j in itertools.combinations(range(n), 2):
        if e.signature(i) == e.signature(j) and target(CASES[i]) != target(CASES[j]):
            pair = (i, j)
            break
    assert pair, "构造失败：初始词汇竟然分得开正反例"
    i, j = pair
    (pos, neg) = (i, j) if target(CASES[i]) else (j, i)
    e.answer(pos, True)
    e.answer(neg, False)
    print(f"初始样本: +{pos} {CASES[pos]}   -{neg} {CASES[neg]}")
    print(f"状态: {e.status().value}   （T2 触发，词汇不足）")

    # 提议：先提三个"看着合理但分不开这一对"的诱饵，最后才提对的。
    # 这模拟一个差的提议器，用来量化"提议质量 → 成本"。
    tail = list(DECOYS.keys()) + [NEEDED[0]]

    proposals = 0
    queries = 0
    while e.status() is Status.INSUFFICIENT:
        if not tail:
            print("提议池耗尽，仍未脱离不足")
            return
        name = tail.pop(0)
        fn = NEEDED[1] if name == NEEDED[0] else DECOYS[name]
        e.propose(name, mask_of(fn))
        proposals += 1
        print(f"  提议 #{proposals}: {name:14s} → 状态 {e.status().value}")

    while e.status() is Status.UNDETERMINED:
        x = e.next_question()
        e.answer(x, target(CASES[x]))
        queries += 1
        print(f"  提问 #{queries}: case {x} {CASES[x]!r} → {target(CASES[x])}")

    # 校验：识别出的外延（含正例的等价类之并）是否就是目标
    derived = 0
    for c in e.classes():
        if c & e.pos:
            derived |= c
    ok = derived == target_mask
    print(f"\n最终状态: {e.status().value}")
    print(f"提议次数 {proposals}，补充提问 {queries}，总成本 {proposals * 20 + queries}（假设提议单价 20）")
    print(f"诱导外延 == 隐藏目标: {ok}  ({bin(derived).count('1')} 个正例)")
    if not ok:
        print(f"  derived={derived:#014b}")
        print(f"  target ={target_mask:#014b}")


if __name__ == "__main__":
    main()
