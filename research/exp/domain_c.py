"""
Domain C: use the behavior of real code as the oracle.

Hidden invariant: the output of `merge_intervals` is **shorter** than the input (a merge happened).
Objects = 12 list-of-intervals test cases; predicates = executable features of a case;
oracle = actually run the function and look at the output length.

The initial vocabulary deliberately lacks the predicate "overlapping intervals exist", so T2 must trigger —
unless the correct predicate is proposed, no number of further case questions can solve it (T6).

Run: python domain_c.py
"""
from __future__ import annotations

import itertools
import random

from engine import Explorer, Status


# ---------- code under test ----------

def merge_intervals(intervals: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Merge overlapping intervals; assumes valid input (requires a <= b and non-empty)."""
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


# ---------- cases ----------

CASES: list[list[tuple[int, int]]] = [
    [],                                  # 0  empty
    [(1, 2)],                            # 1  single
    [(1, 2), (3, 4)],                    # 2  disjoint
    [(1, 3), (2, 4)],                    # 3  overlapping
    [(1, 5), (2, 3)],                    # 4  contained
    [(5, 6), (1, 2)],                    # 5  disjoint, out of order
    [(1, 2), (2, 3)],                    # 6  endpoints touching
    [(-3, -1), (-2, 0)],                 # 7  overlapping negatives
    [(0, 10)],                           # 8  one large interval
    [(1, 2), (4, 5), (7, 8)],            # 9  three disjoint
    [(1, 4), (2, 5), (3, 6)],            # 10 three chained overlaps
    [(1, 2), (2, 3), (3, 4)],            # 11 chained touching
]


def target(case: list[tuple[int, int]]) -> bool:
    """Hidden target: the output is shorter than the input (a merge happened). Empty input raises, so it does not count."""
    if not case:
        return False
    return len(merge_intervals(case)) < len(case)


# ---------- predicates (executable features) ----------

def p_empty(c): return len(c) == 0
def p_single(c): return len(c) == 1
def p_has_negative(c): return any(a < 0 or b < 0 for a, b in c)
def p_long(c): return len(c) >= 3
def p_sorted(c): return list(c) == sorted(c)
def p_width_gt3(c): return any(b - a > 3 for a, b in c)


def p_overlap(c):
    """The predicate actually needed: some pair of intervals overlaps or touches."""
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
NEEDED = ("overlap", p_overlap)   # not in the initial vocabulary


# ---------- decoys: predicates that look plausible but cannot separate the colliding pair ----------

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

    # find a pair of cases with "same V0 signature, different target" as the initial sample
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

    # proposals: first propose three decoys that "look plausible but cannot separate this pair", and only then the right one.
    # This models a bad proposer, to quantify "proposal quality → cost".
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

    # check: is the identified extension (the union of equivalence classes containing positives) exactly the target
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
