"""
Domain A (synthetic formal context) v2: comparing question policies under a **restricted hypothesis class**.

The v1 finding (important): if the hypothesis class is "all Boolean combinations" (any definable set), the candidate set F is
a **product space** decomposed by signature class, asking once per undetermined class suffices, and the expected information gain
is exactly 1 bit for every undetermined class — so the gap between "information gain vs random" only reflects that "random repeats questions about classes already determined",
which is an almost tautological conclusion. The 1.62× of v1 comes from exactly that.

v2 therefore tightens the hypothesis class to **conjunctions of at most k literals** (real concepts are usually short formulas);
F is then no longer a product space, and question selection finally has real quality differences.

Experiments:
  E1  questions needed by four question policies to identify the target under the restricted hypothesis class
  E2  the same instances under "arbitrary Boolean combinations" (showing why tightening is needed)
  E3  hit rate when the budget is 3 questions (practical metric)
  E4  when the vocabulary is insufficient, consistent hypotheses within V0 are always empty (empirical version of T2)

Run:
  python domain_a.py --n 300 --objects 12 --attrs 8 --k 2
"""
from __future__ import annotations

import argparse
import itertools
import math
import random
from dataclasses import dataclass


# ---------- basic objects ----------

def sig(obj: int, attrs: list[int]) -> tuple[bool, ...]:
    return tuple(bool((obj >> a) & 1) for a in attrs)


def group_by_sig(objects: list[int], attrs: list[int]) -> list[list[int]]:
    groups: dict[tuple[bool, ...], list[int]] = {}
    for o in objects:
        groups.setdefault(sig(o, attrs), []).append(o)
    return list(groups.values())


def full_class(objects: list[int], attrs: list[int]) -> list[frozenset[int]]:
    """Arbitrary Boolean combinations = any union of signature groups (product space)."""
    buckets = [frozenset(g) for g in group_by_sig(objects, attrs)]
    out = [frozenset()]
    for b in buckets:
        out = out + [s | b for s in out]
    return out


def conj_class(objects: list[int], attrs: list[int], k: int) -> list[frozenset[int]]:
    """Conjunction of at most k literals. A literal = (attr, value), represented by attr and want."""
    lits = [(a, True) for a in attrs] + [(a, False) for a in attrs]
    out: set[frozenset[int]] = {frozenset(objects)}
    for r in range(1, k + 1):
        for combo in itertools.combinations(lits, r):
            ext = frozenset(o for o in objects
                            if all(bool((o >> a) & 1) == want for a, want in combo))
            if ext:
                out.add(ext)
    return list(out)


def consistent(cands: list[frozenset[int]], pos: set[int], neg: set[int]) -> list[frozenset[int]]:
    return [c for c in cands if pos <= c and not (c & neg)]


# ---------- instances ----------

@dataclass
class Instance:
    objects: list[int]
    attrs: list[int]           # V0
    extra: int                 # distinguishing attribute not in V0
    target: frozenset[int]
    pos: set[int]
    neg: set[int]


def make_instance(rng: random.Random, n_obj: int, n_attr: int) -> Instance | None:
    for _ in range(300):
        rows = list(dict.fromkeys(rng.getrandbits(n_attr) for _ in range(n_obj)))
        if len(rows) < max(6, n_obj // 2):
            continue
        a_star = rng.randrange(n_attr)
        others = [a for a in range(n_attr) if a != a_star]
        target = frozenset(o for o in rows if (o >> a_star) & 1)
        if not target or len(target) == len(rows):
            continue
        pair = None
        for x, y in itertools.combinations(rows, 2):
            if sig(x, others) == sig(y, others) and ((x >> a_star) & 1) != ((y >> a_star) & 1):
                pair = (x, y)
                break
        if pair is None:
            continue
        x, y = pair
        inside, outside = (x, y) if (x >> a_star) & 1 else (y, x)
        pos, neg = {inside}, {outside}
        rp = [o for o in target if o not in pos]
        rn = [o for o in rows if o not in target and o not in neg]
        if rp:
            pos.add(rng.choice(rp))
        if rn:
            neg.add(rng.choice(rn))
        return Instance(rows, others, a_star, target, pos, neg)
    return None


# ---------- question policy ----------

def entropy(f: list[frozenset[int]]) -> float:
    return math.log2(len(f)) if f else 0.0


def gain_of(f: list[frozenset[int]], x: int) -> float:
    if not f:
        return -1.0
    yes = sum(1 for c in f if x in c)
    if yes == 0 or yes == len(f):
        return 0.0
    p = yes / len(f)
    post = p * math.log2(yes) + (1 - p) * math.log2(len(f) - yes)
    return entropy(f) - post


def pick(policy: str, f: list[frozenset[int]], pool: list[int], rng: random.Random) -> int:
    if policy == "gain":
        return max(pool, key=lambda x: (gain_of(f, x), rng.random()))
    if policy == "balance":
        return max(pool, key=lambda x: (min(sum(1 for c in f if x in c),
                                           sum(1 for c in f if x not in c)), rng.random()))
    if policy == "first":
        return pool[0]
    if policy == "rand_inf":
        # exclude only objects that carry no information even when asked; isolates "avoid redundant questions" from "better vs worse selection"
        inf = [o for o in pool if gain_of(f, o) > 0]
        return rng.choice(inf) if inf else rng.choice(pool)
    return rng.choice(pool)


# ---------- identification ----------

def identify(inst: Instance, attrs: list[int], cands: list[frozenset[int]],
             policy: str, rng: random.Random, budget: int | None = None) -> tuple[int | None, bool]:
    """Return (number of questions, whether the target was hit). Hit = all consistent candidates share the same extension and it equals target."""
    f = consistent(cands, inst.pos, inst.neg)
    if not f:
        return None, False
    asked: set[int] = set()
    q = 0
    while len({c for c in f}) > 1:
        pool = [o for o in inst.objects if o not in asked]
        if not pool:
            break
        if budget is not None and q >= budget:
            break
        x = pick(policy, f, pool, rng)
        asked.add(x)
        q += 1
        f = [c for c in f if (x in c) == (x in inst.target)]
        if not f:
            return None, False
    hit = len(f) >= 1 and all(c == inst.target for c in f)
    return q, hit


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=300)
    ap.add_argument("--objects", type=int, default=12)
    ap.add_argument("--attrs", type=int, default=8)
    ap.add_argument("--k", type=int, default=2)
    ap.add_argument("--seed", type=int, default=11)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    insts = [i for i in (make_instance(rng, args.objects, args.attrs) for _ in range(args.n)) if i]
    print(f"instances={len(insts)}  objects≈{args.objects}  attrs={args.attrs}  k={args.k}")

    # E4: consistent conjunctive hypotheses within V0 are empty
    empty = sum(1 for i in insts if not consistent(conj_class(i.objects, i.attrs, args.k), i.pos, i.neg))
    print(f"E4  V0 内一致合取假设为空: {empty}/{len(insts)} ({100*empty/len(insts):.1f}%)")

    # E2: arbitrary Boolean combinations (the v1 setting)
    print("\nE2  假设类 = 任意布尔组合")
    for policy in ("gain", "balance", "rand_inf", "random", "first"):
        qs, hits = [], 0
        for i in insts:
            attrs = i.attrs + [i.extra]
            q, hit = identify(i, attrs, full_class(i.objects, attrs), policy, rng)
            if q is not None:
                qs.append(q)
            hits += hit
        mean = sum(qs) / len(qs) if qs else float("nan")
        print(f"    {policy:8s} mean_queries={mean:5.2f}  hit={hits}/{len(insts)}")

    # E1: restricted conjunctive class
    print("\nE1  假设类 = ≤%d 文字合取" % args.k)
    for policy in ("gain", "balance", "rand_inf", "random", "first"):
        qs, hits = [], 0
        for i in insts:
            attrs = i.attrs + [i.extra]
            q, hit = identify(i, attrs, conj_class(i.objects, attrs, args.k), policy, rng)
            if q is not None:
                qs.append(q)
            hits += hit
        mean = sum(qs) / len(qs) if qs else float("nan")
        print(f"    {policy:8s} mean_queries={mean:5.2f}  hit={hits}/{len(insts)}")

    # E3: budget = 3
    print("\nE3  预算=3 次提问的命中率（合取类）")
    for policy in ("gain", "balance", "rand_inf", "random", "first"):
        hits = 0
        for i in insts:
            attrs = i.attrs + [i.extra]
            _, hit = identify(i, attrs, conj_class(i.objects, attrs, args.k), policy, rng, budget=3)
            hits += hit
        print(f"    {policy:8s} hit@3 = {hits}/{len(insts)} ({100*hits/len(insts):.0f}%)")


if __name__ == "__main__":
    main()

