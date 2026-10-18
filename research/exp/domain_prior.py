"""
E5: question policies under a prior (Occam weights).

The conclusion of E1/E2 is "information gain ≈ asking a random question that still carries information" — under a uniform posterior this is almost inevitable:
each undetermined class in the candidate set holds half the mass, so any question that splits them is worth 1 bit.
E5 changes one condition: add a prior p(S) ∝ 2^{-|S|} to the candidates (shorter conjunctions are more likely).
The posterior is then non-uniform, and only then can the expected information gain of different questions really differ.

Baselines:
  gain_w    pick by the expected drop in posterior entropy (true Bayesian active learning)
  balance   pick only by "splits evenly", ignoring the weights
  rand_inf  random among objects that carry information
"""
from __future__ import annotations

import itertools
import math
import random

from domain_a import Instance, conj_class, make_instance


def weighted_conj(objects: list[int], attrs: list[int], k: int) -> list[tuple[frozenset[int], float]]:
    """Return (extension, weight) with weight = 2^{-literal count}; weights of identical extensions add up."""
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
    # hit: are all remaining candidates (possibly several with the same extension) equal to target
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
