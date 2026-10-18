"""
Empirical version of proposal quality q: in the real loop, is total cost ≈ r/q?

Model:
  - each instance starts with a vocabulary missing a*, and the target S = ext(a*), so the status starts as insufficient.
  - each proposal costs r=20: with probability q it proposes the "correct predicate" (the extension of a*), otherwise a decoy
    (built from a random combination of existing predicates — by T2 it cannot separate same-signature positives and negatives, so it is necessarily useless).
  - once the correct predicate is in place, ask one representative of each undecided class per round (cost 1 per round) until identified.

To verify: total cost ≈ r/q + rounds, and the "number of wasted proposals" ≈ 1/q - 1.

Run: python proposer_quality.py
"""
from __future__ import annotations

import argparse
import random
from statistics import mean

from domain_a import make_instance
from engine import Explorer, Status

R = 20  # unit price of one proposal


def run_once(inst, q: float, rng: random.Random, max_proposals: int = 40):
    objs = inst.objects
    n = len(objs)
    idx = {o: i for i, o in enumerate(objs)}
    e = Explorer(n)
    for name, fn in enumerate(inst.attrs):
        mask = sum(1 << idx[o] for o in objs if (o >> fn) & 1)
        e.propose(f"v{fn}", mask)

    for o in inst.pos:
        e.answer(idx[o], True)
    for o in inst.neg:
        e.answer(idx[o], False)

    target_mask = sum(1 << idx[o] for o in objs if o in inst.target)
    proposals = 0
    rounds = 0
    while proposals < max_proposals:
        st = e.status()
        if st is Status.INSUFFICIENT:
            proposals += 1
            if rng.random() < q:
                e.propose("target", target_mask)
            else:
                # decoy: a random combination of existing predicates; by T2 it can never separate a same-signature pair
                picks = rng.sample(inst.attrs, k=min(2, len(inst.attrs)))
                mask = 0
                for p in picks:
                    mask |= sum(1 << idx[o] for o in objs if (o >> p) & 1)
                e.propose(f"decoy{proposals}", mask)
            continue
        if st is Status.IDENTIFIED:
            derived = 0
            for c in e.classes():
                if c & e.pos:
                    derived |= c
            return R * proposals + rounds, proposals, rounds, derived == target_mask
        x = e.next_question()
        if x is None:
            return R * proposals + rounds, proposals, rounds, False
        e.answer(x, objs[x] in inst.target)
        rounds += 1
    return R * proposals + rounds, proposals, rounds, False


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=200)
    ap.add_argument("--seed", type=int, default=41)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    insts = [i for i in (make_instance(rng, 12, 7) for _ in range(args.n)) if i]
    print(f"instances={len(insts)}  r={R}")
    print(f"{'q':>6} {'cost':>8} {'theory r/q':>9} {'props':>6} {'waste':>6} {'rounds':>6} {'succ':>6}")

    for q in (1 / 8, 1 / 4, 1 / 2, 1.0):
        res = [run_once(i, q, rng) for i in insts]
        cost = mean(r[0] for r in res)
        props = mean(r[1] for r in res)
        rounds = mean(r[2] for r in res)
        succ = mean(1.0 if r[3] else 0.0 for r in res)
        print(f"{q:6.2f} {cost:8.1f} {R/q:9.1f} {props:6.2f} {props-1:6.2f} {rounds:6.2f} {succ:6.2f}")


if __name__ == "__main__":
    main()
