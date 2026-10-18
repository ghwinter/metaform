"""
Per-object posterior: use Beta-Binomial instead of a fixed vote-difference threshold.

Each object x has (yes, no) votes, prior Beta(1,1), posterior Beta(yes+1, no+1).
For integer parameters there is a closed form: n = yes+no+1,
    P(p > 0.5) = Σ_{k=0}^{yes} C(n,k) / 2^n
Strong positive: P ≥ 1-δ; strong negative: P ≤ δ.

Compare against fixed θ=3 / θ=5 to see whether it can repair the distortion at ε=0.3, and at what cost in rounds.

Run: python posterior_gate.py
"""
from __future__ import annotations

import argparse
import math
import random
from statistics import mean

from domain_a import group_by_sig, make_instance


def prob_above_half(yes: int, no: int) -> float:
    n = yes + no + 1
    total = 2 ** n
    acc = sum(math.comb(n, k) for k in range(0, yes + 1))
    return acc / total


def run_once(inst, eps: float, policy: str, rng: random.Random, max_rounds: int = 120):
    delta = 0.05
    theta_fixed = 3 if policy == "theta3" else 5
    attrs = list(inst.attrs)
    objs = inst.objects
    truth = {o: o in inst.target for o in objs}
    votes = {o: [0, 0] for o in objs}
    rounds = 0

    init = 3 if policy != "theta5" else 5
    for _ in range(init):
        rounds += 1
        for o in inst.pos | inst.neg:
            ans = truth[o] if rng.random() >= eps else not truth[o]
            votes[o][0 if ans else 1] += 1

    def strong():
        sp, sn = set(), set()
        for o in objs:
            y, n = votes[o]
            if y + n == 0:
                continue
            p = prob_above_half(y, n)
            if policy in ("theta3", "theta5"):
                if y - n >= theta_fixed:
                    sp.add(o)
                elif n - y >= theta_fixed:
                    sn.add(o)
            else:
                if p >= 1 - delta and y > n:
                    sp.add(o)
                elif p <= delta and n > y:
                    sn.add(o)
        return sp, sn

    pool = ["decoy1", "decoy2", "TRUE"]
    proposals = 0
    wasted = 0
    confirm_left = 0

    def ask(batch):
        nonlocal rounds
        if not batch:
            return
        rounds += 1
        for o in batch:
            ans = truth[o] if rng.random() >= eps else not truth[o]
            votes[o][0 if ans else 1] += 1

    while rounds < max_rounds:
        sp, sn = strong()
        cls = group_by_sig(objs, attrs)
        conflict = [c for c in cls if any(o in c for o in sp) and any(o in c for o in sn)]
        if conflict:
            if confirm_left > 0:
                confirm_left -= 1
                ask([o for c in conflict for o in c])
                continue
            if not pool:
                return rounds, proposals, wasted, False
            name = pool.pop(0)
            proposals += 1
            confirm_left = 2
            if name != "TRUE":
                wasted += 1
                continue
            attrs = attrs + [inst.extra]
            continue

        pending = [o for c in cls for o in c
                   if o not in sp and o not in sn and (votes[o][0] + votes[o][1]) > 0]
        if pending:
            ask(pending)
            continue

        reps = [c[0] for c in cls if not any(o in c for o in sp | sn)]
        if reps:
            ask(reps)
            continue

        pred = {}
        for c in cls:
            pos_in = any(o in c for o in sp)
            neg_in = any(o in c for o in sn)
            if not (pos_in or neg_in):
                return rounds, proposals, wasted, False
            for o in c:
                pred[o] = True if pos_in else False
        ok = all(pred[o] == truth[o] for o in objs)
        return rounds, proposals, wasted, ok

    return rounds, proposals, wasted, False


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=150)
    ap.add_argument("--seed", type=int, default=29)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    insts = [i for i in (make_instance(rng, 12, 7) for _ in range(args.n)) if i]
    print(f"instances={len(insts)}  delta=0.05")
    print(f"{'eps':>5} {'policy':>7} | {'成功':>6} {'轮数':>6} {'提议':>6}")

    for eps in (0.10, 0.20, 0.30):
        for policy in ("theta3", "theta5", "posterior"):
            res = [run_once(i, eps, policy, rng) for i in insts]
            succ = mean(1.0 if r[3] else 0.0 for r in res)
            print(f"{eps:5.2f} {policy:>7} | {succ:6.2f} {mean(r[0] for r in res):6.2f} "
                  f"{mean(r[1] for r in res):6.2f}")


if __name__ == "__main__":
    main()
