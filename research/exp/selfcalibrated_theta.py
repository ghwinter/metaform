"""
用「估出来的 ε」选 θ，比「用真实 ε」差多少？

  true_theta  直接拿实验设定的 ε 去映射（上界，作弊）
  hat_theta   只用票数估计 ε̂ = mean(min(yes,no)/(yes+no))，再映射

两者用同一张映射表（≤0.1→3，≥0.2→5，否则 4）。
若 hat 与 true 的差距 <3%，说明不必知道真实错误率。

运行：python selfcalibrated_theta.py
"""
from __future__ import annotations

import argparse
import random
from statistics import mean

from domain_a import group_by_sig, make_instance


def theta_from_eps(e: float) -> int:
    if e <= 0.10:
        return 3
    if e >= 0.20:
        return 5
    return 4


def _minority_rates(votes: dict[int, list[int]]) -> list[float]:
    vals = []
    for y, n in votes.values():
        t = y + n
        if t:
            vals.append(min(y, n) / t)
    return vals


def eps_hat(votes: dict[int, list[int]]) -> float:
    vals = _minority_rates(votes)
    return mean(vals) if vals else 0.0


def eps_hat_q(votes: dict[int, list[int]], q: float = 0.75) -> float:
    """少数票占比的 75 分位：少数票比例会低估 ε，取高分位更保守。"""
    vals = sorted(_minority_rates(votes))
    if not vals:
        return 0.0
    idx = min(len(vals) - 1, int(q * (len(vals) - 1)))
    return vals[idx]


def run_once(inst, eps: float, policy: str, rng: random.Random, max_rounds: int = 90):
    attrs = list(inst.attrs)
    objs = inst.objects
    truth = {o: o in inst.target for o in objs}
    votes = {o: [0, 0] for o in objs}
    theta = 3
    rounds = 0

    for _ in range(theta):
        rounds += 1
        for o in inst.pos | inst.neg:
            ans = truth[o] if rng.random() >= eps else not truth[o]
            votes[o][0 if ans else 1] += 1

    def strong(t):
        sp = {o for o in objs if votes[o][0] - votes[o][1] >= t}
        sn = {o for o in objs if votes[o][1] - votes[o][0] >= t}
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
        # 每轮开始时按策略更新 θ
        if policy == "true":
            theta = theta_from_eps(eps)
        elif policy == "hat_q":
            theta = theta_from_eps(eps_hat_q(votes))
        else:
            theta = theta_from_eps(eps_hat(votes))

        sp, sn = strong(theta)
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
    print(f"instances={len(insts)}")
    print(f"{'eps':>5} {'policy':>9} | {'成功':>6} {'轮数':>6} {'提议':>6}")

    for eps in (0.05, 0.10, 0.20, 0.30):
        line = {}
        for policy in ("true", "hat", "hat_q"):
            res = [run_once(i, eps, policy, rng) for i in insts]
            succ = mean(1.0 if r[3] else 0.0 for r in res)
            rds = mean(r[0] for r in res)
            line[policy] = (succ, rds)
            print(f"{eps:5.2f} {policy:>9} | {succ:6.2f} {rds:6.2f} {mean(r[1] for r in res):6.2f}")
        for p in ("hat", "hat_q"):
            ds = line["true"][0] - line[p][0]
            dr = line[p][1] - line["true"][1]
            print(f"   {p:>5} 差 | 成功 {ds:+.3f}   轮数 {dr:+.2f}")


if __name__ == "__main__":
    main()

