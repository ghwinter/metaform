"""
Algorithmic side of T7: proposals = set cover over colliding pairs.

Question: given a batch of candidate predicates (each covering some colliding pairs), how many are needed to cover all of them?
  greedy  each step picks the one covering the most (the pipeline in this paper)
  random  take them in random order until everything is covered
  optimal brute-force the smallest subset (k is tiny, so feasible), used to measure the greedy approximation ratio

Run: python greedy_cover.py --n 300 --pool 12
"""
from __future__ import annotations

import argparse
import itertools
import random
from statistics import mean


def greedy(cover_sets: list[set], universe: set) -> int | None:
    remaining = set(universe)
    used = 0
    while remaining:
        best = max(cover_sets, key=lambda s: len(s & remaining))
        if not s_ok(best, remaining):
            return None
        remaining -= best
        used += 1
    return used


def s_ok(s: set, remaining: set) -> bool:
    return bool(s & remaining)


def random_order(cover_sets: list[set], universe: set, rng: random.Random) -> int | None:
    order = list(cover_sets)
    rng.shuffle(order)
    covered: set = set()
    used = 0
    for s in order:
        used += 1
        covered |= s
        if covered >= universe:
            return used
    return None


def optimal(cover_sets: list[set], universe: set) -> int | None:
    for r in range(0, len(cover_sets) + 1):
        for combo in itertools.combinations(cover_sets, r):
            if set().union(*combo) >= universe if combo else not universe:
                return r
    return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--pool", type=int, default=10)
    ap.add_argument("--objects", type=int, default=12)
    ap.add_argument("--seed", type=int, default=5)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    g_vals, r_vals, o_vals = [], [], []
    unsolved = 0

    for _ in range(args.n):
        # colliding pairs: pos × neg (within the same equivalence class)
        n_pos, n_neg = 3, 3
        pairs = list(itertools.product(range(n_pos), range(n_pos, n_pos + n_neg)))
        universe = set(pairs)
        # candidate predicates: random values on objects, covering the pairs they can separate
        cover_sets = []
        for _ in range(args.pool):
            mask = [rng.random() < 0.5 for _ in range(args.objects)]
            cov = {(a, b) for (a, b) in pairs if mask[a] != mask[b]}
            if cov:
                cover_sets.append(cov)
        if not cover_sets or set().union(*cover_sets) < universe:
            unsolved += 1
            continue
        g = greedy(cover_sets, universe)
        r = random_order(cover_sets, universe, rng)
        o = optimal(cover_sets, universe)
        if g is None or r is None or o is None:
            unsolved += 1
            continue
        g_vals.append(g)
        r_vals.append(r)
        o_vals.append(o)

    print(f"instances={len(g_vals)}  unsolved(不可覆盖)={unsolved}  pool={args.pool}")
    print(f"greedy  mean={mean(g_vals):.2f}")
    print(f"random  mean={mean(r_vals):.2f}")
    print(f"optimal mean={mean(o_vals):.2f}")
    print(f"greedy/optimal = {mean(g_vals)/mean(o_vals):.2f}")
    print(f"random/greedy  = {mean(r_vals)/mean(g_vals):.2f}")


if __name__ == "__main__":
    main()
