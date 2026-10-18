"""
噪声下的完整循环：θ 门到底值不值。

每轮对一批对象各问一次，每答以概率 ε 翻转，记一票。
判定用票差阈值 θ：强正例 yes-no ≥ θ，强反例 no-yes ≥ θ。

三条规则（上一版踩过的坑都写在这里）：

1. **种子也必须确认到 θ 票**——否则 θ 越大，越容易把一个弱标签当成确证，
   真撞车就"看不见"了。
2. **一个类里只要还有"投过票但未达 θ"的成员，这个类就不算已决**，必须继续确认。
   否则会出现"类里一个强标签 + 一个弱反标签"被静默接受的情况。
3. **撞车先复核再换词表**：冲突可能只是噪声，先补票 `confirm_left` 轮，仍冲突才提议。
   提议池前两个是诱饵，第三个才是正确谓词。

指标：成功率（整条外延都对）、询问轮数、提议次数、被浪费的提议数。

运行：python noisy_loop.py
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

        # 规则 3：冲突
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

        # 规则 2：类里还有投过票但未达 θ 的成员 → 先补票
        pending = [
            o
            for c in cls
            for o in c
            if o not in sp and o not in sn and (votes[o][0] + votes[o][1]) > 0
        ]
        if pending:
            ask(pending)
            continue

        # 未决类：问代表
        reps = [c[0] for c in cls if not any(o in c for o in sp | sn)]
        if reps:
            ask(reps)
            continue

        # 规则 1 的收尾：每个类都必须有强标签，且外延整体正确
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
