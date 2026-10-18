"""
The full loop under noise: is the θ gate worth it in the end?

Each round asks a batch of objects once; each answer flips with probability ε and records one vote.
The decision uses the vote-difference threshold θ: strong positive yes-no ≥ θ, strong negative no-yes ≥ θ.

Three rules (every trap hit by the previous version is written here):

1. **The seed must also be confirmed up to θ votes** — otherwise the larger θ is, the easier a weak label is taken as settled,
   and a real collision becomes "invisible".
2. **As long as a class still has members with "votes cast but below θ", that class is not decided**, and confirmation must continue.
   Otherwise a "strong positive label + weak negative label in one class" can be silently accepted.
3. **On a collision, re-check before swapping the vocabulary**: the conflict may just be noise, so first top up votes for `confirm_left` rounds, and propose only if the conflict persists.
   The first two entries in the proposal pool are decoys; the third is the correct predicate.

Metrics: success rate (the whole extension is correct), number of question rounds, number of proposals, number of wasted proposals.

Run: python noisy_loop.py
"""
from __future__ import annotations

import argparse
import random
from statistics import mean

from domain_a import group_by_sig, make_instance


def run_once(inst, eps: float, theta: int, rng: random.Random, max_rounds: int = 80):
    attrs = list(inst.attrs)
    objs = inst.objects
    truth = {o: o in inst.target for o in objs}

    votes = {o: [0, 0] for o in objs}
    rounds = 0
    for _ in range(theta):
        rounds += 1
        for o in inst.pos | inst.neg:
            ans = truth[o] if rng.random() >= eps else not truth[o]
            votes[o][0 if ans else 1] += 1

    pool = ["decoy1", "decoy2", "TRUE"]
    proposals = 0
    wasted = 0
    confirm_left = 0

    def strong():
        sp = {o for o in objs if votes[o][0] - votes[o][1] >= theta}
        sn = {o for o in objs if votes[o][1] - votes[o][0] >= theta}
        return sp, sn

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

        # rule 3: collision
        conflict_classes = [c for c in cls if any(o in c for o in sp) and any(o in c for o in sn)]
        if conflict_classes:
            if confirm_left > 0:
                confirm_left -= 1
                ask([o for c in conflict_classes for o in c])
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

        # rule 2: the class still has members that voted but are below θ → top up votes first
        pending = [
            o
            for c in cls
            for o in c
            if o not in sp and o not in sn and (votes[o][0] + votes[o][1]) > 0
        ]
        if pending:
            ask(pending)
            continue

        # undecided classes: ask a representative
        reps = [c[0] for c in cls if not any(o in c for o in sp | sn)]
        if reps:
            ask(reps)
            continue

        # wrap-up for rule 1: every class must have a strong label, and the whole extension must be correct
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
    ap.add_argument("--objects", type=int, default=12)
    ap.add_argument("--attrs", type=int, default=7)
    ap.add_argument("--seed", type=int, default=29)
    args = ap.parse_args()

    rng = random.Random(args.seed)
    insts = [i for i in (make_instance(rng, args.objects, args.attrs) for _ in range(args.n)) if i]
    print(f"instances={len(insts)}  objects≈{args.objects}  attrs={args.attrs}")
    print(f"{'eps':>5} {'theta':>5} | {'成功':>6} {'轮数':>6} {'提议':>6} {'浪费':>6}")

    for eps in (0.0, 0.1, 0.2):
        for theta in (1, 3, 5):
            res = [run_once(i, eps, theta, rng) for i in insts]
            succ = mean(1.0 if r[3] else 0.0 for r in res)
            print(f"{eps:5.2f} {theta:5d} | {succ:6.2f} {mean(r[0] for r in res):6.2f} "
                  f"{mean(r[1] for r in res):6.2f} {mean(r[2] for r in res):6.2f}")


if __name__ == "__main__":
    main()
