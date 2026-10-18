"""
T6 的经验版：给出更多例子，会不会"修复"词汇不足？

直觉（也是 T6）：不会。更多例子只增加约束、不改变可用语言。
更强的断言：更多例子会**暴露**先前没被发现的词汇不足——即 F 由非空变空的概率随例子数上升。

做法：构造隐藏目标 S = 属性 a* 的外延，但初始词汇 V0 不含 a*。
按 k = 1..8 抽样，每次从 S 内抽 ceil(k/2) 个正例、S 外抽 rest 个负例，
统计"V0 内一致的合取假设为空"的比例。

若无差别的现象是"一开始就空"，那说明这些实例太容易暴露；
按题设，有些实例在 k 小的时候是**不空**的（正负例恰好签名不同），随 k 增大才撞车。
"""
from __future__ import annotations

import random

from domain_a import Instance, conj_class, make_instance
from domain_a import consistent


def main() -> None:
    rng = random.Random(31)
    insts = [i for i in (make_instance(rng, 14, 8) for _ in range(400)) if i]
    print(f"instances={len(insts)}, objects≈14, attrs=8 (a* 不在 V0)")

    ks = [1, 2, 3, 4, 6, 8, 10]
    print("\nk  examples    P(F empty in V0)")
    for k in ks:
        empty = 0
        total = 0
        for i in insts:
            n_pos = max(1, k // 2)
            n_neg = max(1, k - n_pos)
            pos_pool = list(i.target)
            neg_pool = [o for o in i.objects if o not in i.target]
            if len(pos_pool) < n_pos or len(neg_pool) < n_neg:
                continue
            for _ in range(20):  # 每个实例重复抽样，降低方差
                pos = set(rng.sample(pos_pool, n_pos))
                neg = set(rng.sample(neg_pool, n_neg))
                cands = conj_class(i.objects, i.attrs, 2)
                total += 1
                if not consistent(cands, pos, neg):
                    empty += 1
        print(f"{k:2d}  {k:8d}    {100*empty/total:5.1f}%   ({empty}/{total})")


if __name__ == "__main__":
    main()
