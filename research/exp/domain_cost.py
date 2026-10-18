"""
成本实验：对象查询 vs 谓词提议，谁决定总成本？

模型（单缺失谓词）：
  - 目标 S = 某属性 a* 的外延，a* 不在初始词汇 V0。
  - 对象查询成本 1，谓词提议成本 r，提议命中 a* 的概率 q（未命中就白花 r）。
  - "识别"= 知道每个对象是否属于 S。

两条策略：
  ask_first    先把 V0 下的每个未定签名类问一个代表，再提议，直到命中，再补齐剩余的类
  propose_first 直接提议直到命中，再问所有未定类

要验证的断言（C3'）：**总成本的量级由提议质量 (r/q) 决定，停止规则只是二阶项。**
"""
from __future__ import annotations

import random
from statistics import mean

from domain_a import Instance, make_instance


def classes(objects: list[int], attrs: list[int]) -> dict[tuple[bool, ...], list[int]]:
    out: dict[tuple[bool, ...], list[int]] = {}
    for o in objects:
        key = tuple(bool((o >> a) & 1) for a in attrs)
        out.setdefault(key, []).append(o)
    return out


def free_reps(objects: list[int], attrs: list[int], known: set[int]) -> list[int]:
    """每个还没被覆盖的签名类，取一个代表。"""
    reps = []
    for _key, members in classes(objects, attrs).items():
        if any(m in known for m in members):
            continue
        reps.append(members[0])
    return reps


def propose_until_hit(rng: random.Random, q: float) -> int:
    n = 0
    while True:
        n += 1
        if rng.random() < q:
            return n


def total_cost(inst: Instance, policy: str, r: float, q: float, rng: random.Random) -> float:
    known: set[int] = set(inst.pos) | set(inst.neg)
    cost = 0.0
    if policy == "ask_first":
        reps = free_reps(inst.objects, inst.attrs, known)
        known |= set(reps)
        cost += len(reps)
    cost += propose_until_hit(rng, q) * r
    attrs1 = inst.attrs + [inst.extra]
    cost += len(free_reps(inst.objects, attrs1, known))
    return cost


def main() -> None:
    rng = random.Random(41)
    insts = [i for i in (make_instance(rng, 14, 8) for _ in range(300)) if i]
    print(f"instances={len(insts)}  objects≈14  attrs=8\n")
    for q in (1 / 8, 1 / 4, 1 / 2):
        print(f"q={q:.3f} （平均 {1/q:.1f} 次提议命中）")
        for r in (5, 20, 100):
            a = mean(total_cost(i, "ask_first", r, q, rng) for i in insts)
            b = mean(total_cost(i, "propose_first", r, q, rng) for i in insts)
            print(f"   r={r:3d}  ask_first={a:7.2f}  propose_first={b:7.2f}  diff={b-a:+6.2f}  ({(b-a)/a*100:+.1f}%)")
        print()


if __name__ == "__main__":
    main()
