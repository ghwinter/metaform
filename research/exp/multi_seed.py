"""
多 seed 方差：给论文里的关键对比补上均值 ± 标准差。

覆盖两项：
  A. 提问策略（gain / rand_inf / random），合取类 k=2，objects=12，attrs=8
  B. 覆盖策略（greedy / optimal / random），3×3 对、10 个随机候选

运行：python multi_seed.py --seeds 1 2 3 4 5
"""
from __future__ import annotations

import argparse
import itertools
import random
import statistics

from domain_a import conj_class, identify, make_instance


# ---------- A：提问策略 ----------

def run_policy(seed: int, policy: str, n: int, objects: int, attrs: int, k: int) -> float:
    rng = random.Random(seed)
    insts = [i for i in (make_instance(rng, objects, attrs) for _ in range(n)) if i]
    qs = []
    for inst in insts:
        a = inst.attrs + [inst.extra]
        q, _ = identify(inst, a, conj_class(inst.objects, a, k), policy, rng)
        if q is not None:
            qs.append(q)
    return statistics.mean(qs) if qs else float("nan")


# ---------- B：覆盖策略 ----------

def greedy_size(cover_sets, universe):
    rem = set(universe)
    used = 0
    while rem:
        best = max(cover_sets, key=lambda s: len(s & rem))
        if not (best & rem):
            return None
        rem -= best
        used += 1
    return used


def random_size(cover_sets, universe, rng):
    order = list(cover_sets)
    rng.shuffle(order)
    cov = set()
    for i, s in enumerate(order, 1):
        cov |= s
        if cov >= universe:
            return i
    return None


def optimal_size(cover_sets, universe):
    for r in range(len(cover_sets) + 1):
        for combo in itertools.combinations(cover_sets, r):
            if set().union(*combo) >= universe:
                return r
    return None


def run_cover(seed: int, n: int, pool: int, objects: int) -> tuple[float, float, float]:
    rng = random.Random(seed)
    g, r_, o = [], [], []
    for _ in range(n):
        pairs = list(itertools.product(range(3), range(3, 6)))
        universe = set(pairs)
        covers = []
        for _ in range(pool):
            mask = [rng.random() < 0.5 for _ in range(objects)]
            cov = {(a, b) for (a, b) in pairs if mask[a] != mask[b]}
            if cov:
                covers.append(cov)
        if not covers or set().union(*covers) < universe:
            continue
        x, y, z = greedy_size(covers, universe), random_size(covers, universe, rng), optimal_size(covers, universe)
        if None in (x, y, z):
            continue
        g.append(x); r_.append(y); o.append(z)
    return statistics.mean(g), statistics.mean(r_), statistics.mean(o)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", type=int, nargs="+", default=[1, 2, 3, 4, 5])
    ap.add_argument("--n", type=int, default=150)
    ap.add_argument("--objects", type=int, default=12)
    ap.add_argument("--attrs", type=int, default=8)
    ap.add_argument("--k", type=int, default=2)
    ap.add_argument("--pool", type=int, default=10)
    args = ap.parse_args()

    print("=== A 提问策略（合取 k=%d, n=%d） ===" % (args.k, args.n))
    for policy in ("gain", "rand_inf", "random"):
        vals = [run_policy(s, policy, args.n, args.objects, args.attrs, args.k) for s in args.seeds]
        print(f"  {policy:9s} {statistics.mean(vals):6.2f} ± {statistics.stdev(vals):4.2f}   "
              f"seeds={['%.2f' % v for v in vals]}")

    print("=== B 覆盖策略（pool=%d, n=%d） ===" % (args.pool, args.n))
    res = [run_cover(s, args.n, args.pool, args.objects) for s in args.seeds]
    names = ("greedy", "random", "optimal")
    for i, name in enumerate(names):
        vals = [r[i] for r in res]
        print(f"  {name:9s} {statistics.mean(vals):6.2f} ± {statistics.stdev(vals):4.2f}   "
              f"seeds={['%.2f' % v for v in vals]}")


if __name__ == "__main__":
    main()
