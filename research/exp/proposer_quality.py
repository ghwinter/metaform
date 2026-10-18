"""
提议质量 q 的实证版：在真实循环里，总成本是否 ≈ r/q？

模型：
  - 每个实例初始词汇缺 a*，目标 S = ext(a*)，因此状态一开始是 insufficient。
  - 每次提议花 r=20：以概率 q 提出"正确谓词"（a* 的外延），否则提出一个诱饵
    （由现有谓词随机组合而成——按 T2，它分不开同签名的正反例，必然无效）。
  - 正确谓词到位后，逐轮问未决类的代表（每轮成本 1），直到 identified。

要验的：总成本 ≈ r/q + 轮数，且"浪费的提议数" ≈ 1/q - 1。

运行：python proposer_quality.py
"""
from __future__ import annotations

import argparse
import random
from statistics import mean

from domain_a import make_instance
from engine import Explorer, Status

R = 20  # 一次提议的单价


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
                # 诱饵：现有谓词的随机组合，按 T2 一定分不开同签名对
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
    print(f"{'q':>6} {'总成本':>8} {'理论 r/q':>9} {'提议':>6} {'浪费':>6} {'轮数':>6} {'成功':>6}")

    for q in (1 / 8, 1 / 4, 1 / 2, 1.0):
        res = [run_once(i, q, rng) for i in insts]
        cost = mean(r[0] for r in res)
        props = mean(r[1] for r in res)
        rounds = mean(r[2] for r in res)
        succ = mean(1.0 if r[3] else 0.0 for r in res)
        print(f"{q:6.2f} {cost:8.1f} {R/q:9.1f} {props:6.2f} {props-1:6.2f} {rounds:6.2f} {succ:6.2f}")


if __name__ == "__main__":
    main()
