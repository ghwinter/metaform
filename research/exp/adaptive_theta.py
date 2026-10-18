"""
自适应 θ：冲突反复出现就加 1，长期无冲突就减 1。

要回答的问题：在 ε=0.2 下，自适应能否用少于固定 θ=5 的轮数达到同样的成功率？
  fixed3   固定 θ=3
  fixed5   固定 θ=5
  adapt    从 3 开始，确认冲突后 +1（上限 6），连续 5 轮无冲突 -1（下限 3）

运行：python adaptive_theta.py
"""
from __future__ import annotations

import argparse
import random
from statistics import mean

from domain_a import group_by_sig, make_instance


def run_once(inst, eps: float, policy: str, rng: random.Random, max_rounds: int = 80):
    attrs = list(inst.attrs)
    objs = inst.objects
    truth = {o: o in inst.target for o in objs}
    votes = {o: [0, 0] for o in objs}

    theta0 = 3 if policy != "fixed5" else 5
    theta = theta0
    cap, floor = 6, theta0
    rounds = 0
    calm = 0  # 连续无冲突轮数

    def strong(t):
        sp = {o for o in objs if votes[o][0] - votes[o][1] >= t}
        sn = {o for o in objs if votes[o][1] - votes[o][0] >= t}
        return sp, sn

    # 种子确认到当前 θ
    for _ in range(theta):
        rounds += 1
        for o in inst.pos | inst.neg:
            ans = truth[o] if rng.random() >= eps else not truth[o]
            votes[o][0 if ans else 1] += 1

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
        sp, sn = strong(theta)
        cls = group_by_sig(objs, attrs)
        conflict_classes = [c for c in cls if any(o in c for o in sp) and any(o in c for o in sn)]

        if conflict_classes:
            calm = 0
            if confirm_left > 0:
                confirm_left -= 1
                ask([o for c in conflict_classes for o in c])
                continue
            # 复核完仍冲突 → 换词表，并把 θ 抬高一点（更保守）
            if policy == "adapt" and theta < cap:
                theta += 1
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

        calm += 1
        if policy == "adapt" and calm >= 5 and theta > floor:
            theta -= 1
            calm = 0

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
    print(f"instances={len(insts)}")
    print(f"{'eps':>5} {'policy':>8} | {'成功':>6} {'轮数':>6} {'提议':>6} {'浪费':>6}")

    for eps in (0.1, 0.2):
        for policy in ("fixed3", "fixed5", "adapt"):
            res = [run_once(i, eps, policy, rng) for i in insts]
            succ = mean(1.0 if r[3] else 0.0 for r in res)
            print(f"{eps:5.2f} {policy:>8} | {succ:6.2f} {mean(r[0] for r in res):6.2f} "
                  f"{mean(r[1] for r in res):6.2f} {mean(r[2] for r in res):6.2f}")


if __name__ == "__main__":
    main()
