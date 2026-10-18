"""
E5：带先验（Occam 权重）时的提问策略。

E1/E2 的结论是"信息增益 ≈ 随机问一个还有信息的问题"——在均匀后验下，这几乎是必然的：
候选集里每个未定类各占一半质量，任何能切开的问题都值 1 bit。
E5 改变一个条件：给候选加先验 p(S) ∝ 2^{-|S|}（越短的合取越可能）。
此时后验不均匀，不同问题的期望信息增益才可能真正不同。

对照：
  gain_w    按后验熵的期望下降选（真正的贝叶斯主动学习）
  balance  只按"切得平均"选，无视权重
  rand_inf 在"有信息"的对象里随机
"""
from __future__ import annotations

import itertools
import math
import random

from domain_a import Instance, conj_class, make_instance


def weighted_conj(objects: list[int], attrs: list[int], k: int) -> list[tuple[frozenset[int], float]]:
    """返回 (外延, 权重)，权重 = 2^{-文字数}，同一外延的权重相加。"""
    lits = [(a, True) for a in attrs] + [(a, False) for a in attrs]
    acc: dict[frozenset[int], float] = {frozenset(objects): 1.0}
    for r in range(1, k + 1):
        for combo in itertools.combinations(lits, r):
            ext = frozenset(o for o in objects
                            if all(bool((o >> a) & 1) == want for a, want in combo))
            if ext:
                acc[ext] = acc.get(ext, 0.0) + 2.0 ** (-r)
    return list(acc.items())


def post_entropy(f: list[tuple[frozenset[int], float]]) -> float:
    z = sum(w for _, w in f)
    if z <= 0:
        return 0.0
    h = 0.0
    for _, w in f:
        p = w / z
        if p > 0:
            h -= p * math.log2(p)
    return h


def gain_w(f: list[tuple[frozenset[int], float]], x: int) -> float:
    h0 = post_entropy(f)
    yes = [(c, w) for c, w in f if x in c]
    no = [(c, w) for c, w in f if x not in c]
    z = sum(w for _, w in f)
    zy = sum(w for _, w in yes)
    zn = z - zy
    if zy <= 0 or zn <= 0:
        return 0.0
    return h0 - (zy / z) * post_entropy(yes) - (zn / z) * post_entropy(no)


def balance_score(f: list[tuple[frozenset[int], float]], x: int) -> float:
    yes = sum(1 for c, _ in f if x in c)
    return min(yes, len(f) - yes)


def run(inst: Instance, k: int, policy: str, rng: random.Random) -> int | None:
    attrs = inst.attrs + [inst.extra]
    f = [(c, w) for c, w in weighted_conj(inst.objects, attrs, k)
         if inst.pos <= c and not (c & inst.neg)]
    if not f:
        return None
    asked: set[int] = set()
    q = 0
    while len(f) > 1:
        pool = [o for o in inst.objects if o not in asked]
        if not pool:
            return None
        if policy == "gain_w":
            x = max(pool, key=lambda o: (gain_w(f, o), rng.random()))
        elif policy == "balance":
            x = max(pool, key=lambda o: (balance_score(f, o), rng.random()))
        else:
            inf = [o for o in pool if gain_w(f, o) > 0]
            x = rng.choice(inf) if inf else rng.choice(pool)
        asked.add(x)
        q += 1
        f = [(c, w) for c, w in f if (x in c) == (x in inst.target)]
        if not f:
            return None
    # 命中：剩下的候选（可能多个外延相同）是否都等于 target
    return q if all(c == inst.target for c, _ in f) else None


def main() -> None:
    rng = random.Random(23)
    insts = [i for i in (make_instance(rng, 12, 8) for _ in range(300)) if i]
    for k in (2, 3):
        print(f"\nE5  先验 p(S) ∝ 2^-|S|，k={k}，n={len(insts)}")
        for policy in ("gain_w", "balance", "rand_inf"):
            qs = []
            fails = 0
            for i in insts:
                q = run(i, k, policy, rng)
                if q is None:
                    fails += 1
                else:
                    qs.append(q)
            mean = sum(qs) / len(qs) if qs else float("nan")
            print(f"    {policy:8s} mean_queries={mean:5.2f}  solved={len(qs)}  fail={fails}")


if __name__ == "__main__":
    main()
