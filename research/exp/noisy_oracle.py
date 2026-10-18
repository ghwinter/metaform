"""
噪声 oracle 下的 T2：从逻辑判定变成统计判定。

oracle 每次回答以概率 ε 翻转。
  A 假触发：词汇其实够用（S 可定义），噪声在同一等价类里造出"正负并存" → 误判 insufficient
  B 漏检：  词汇真的不够（存在真撞车对），噪声把这对的冲突抹平 → 没触发
  C 重复投票：同一对象问 r 次取多数，等效噪声下降，A 与 B 都该改善

运行：python noisy_oracle.py
"""
from __future__ import annotations

import argparse
import itertools
import random

from domain_a import group_by_sig, sig


def noisy_answer(true_label: bool, eps: float, rng: random.Random) -> bool:
    return (not true_label) if rng.random() < eps else true_label


def majority(votes: list[bool]) -> bool:
    return sum(votes) * 2 > len(votes)


def triggered(examples: list[tuple[int, bool]], attrs: list[int], objs: list[int]) -> bool:
    """是否存在某个等价类，里面同时被标注了正例和反例。"""
    for members in group_by_sig(objs, attrs):
        labels = {lab for (o, lab) in examples if o in members}
        if True in labels and False in labels:
            return True
    return False


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--trials", type=int, default=300)
    ap.add_argument("--objects", type=int, default=14)
    ap.add_argument("--attrs", type=int, default=6)
    ap.add_argument("--examples", type=int, default=6)
    ap.add_argument("--seed", type=int, default=17)
    args = ap.parse_args()

    print(f"trials={args.trials} objects={args.objects} attrs={args.attrs} examples={args.examples}")
    print(f"{'eps':>5} {'r':>2} | {'A 假触发':>10} {'B 漏检':>8}")
    rng = random.Random(args.seed)

    for eps in (0.0, 0.05, 0.1, 0.2, 0.3):
        for r in (1, 3, 5):
            false_alarm = 0
            missed = 0
            valid_b = 0
            for _ in range(args.trials):
                objs = list(dict.fromkeys(rng.getrandbits(args.attrs) for _ in range(args.objects)))
                if len(objs) < args.examples + 2:
                    continue
                a_star = rng.randrange(args.attrs)
                others = [a for a in range(args.attrs) if a != a_star]
                truth = {o: bool((o >> a_star) & 1) for o in objs}

                # A：S 由 V0 的两个属性异或定义，因此在 V0 下可定义；
                # 任何"同类正负并存"都只能是噪声造成的。
                truth_a = {o: bool(((o >> others[0]) & 1) ^ ((o >> others[1]) & 1)) for o in objs}
                chosen = rng.sample(objs, args.examples)
                labels = [
                    (o, majority([noisy_answer(truth_a[o], eps, rng) for _ in range(r)]))
                    for o in chosen
                ]
                if triggered(labels, others, objs):
                    false_alarm += 1

                # B：词汇缺 a*，样本里必须包含一对"同签名、真假不同"的对象
                pair = None
                for x, y in itertools.combinations(objs, 2):
                    if sig(x, others) == sig(y, others) and truth[x] != truth[y]:
                        pair = (x, y)
                        break
                if pair is None:
                    continue
                valid_b += 1
                x, y = pair
                rest = [o for o in objs if o not in (x, y)]
                picked = [x, y] + (rng.sample(rest, max(0, args.examples - 2)) if rest else [])
                noisy = [
                    (o, majority([noisy_answer(truth[o], eps, rng) for _ in range(r)]))
                    for o in picked
                ]
                if not triggered(noisy, others, objs):
                    missed += 1

            print(f"{eps:5.2f} {r:2d} | {false_alarm/args.trials:10.3f} {missed/max(1,valid_b):8.3f}")


if __name__ == "__main__":
    main()

