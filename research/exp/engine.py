"""
`engine/src/lib.rs` 的 Python 对照实现：只保留论文需要的那部分语义。

对象用 0..n-1 的整数，集合用整数位掩码。谓词是一对 (名字, 掩码)。
与 Rust 版一一对应：status / next_question / answer / propose / capacity。
"""
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


class Status(Enum):
    INSUFFICIENT = "insufficient"   # 正反例同类：当前词汇已被证明不够（T2）
    IDENTIFIED = "identified"       # 每个等价类都被覆盖
    UNDETERMINED = "undetermined"   # 还有未覆盖的类


@dataclass
class Explorer:
    n: int
    preds: list[tuple[str, int]] = field(default_factory=list)
    pos: int = 0
    neg: int = 0

    # --- 基本操作 ---

    def signature(self, x: int) -> tuple[bool, ...]:
        bit = 1 << x
        return tuple(bool(mask & bit) for _, mask in self.preds)

    def classes(self) -> list[int]:
        groups: dict[tuple[bool, ...], int] = {}
        for x in range(self.n):
            key = self.signature(x)
            groups[key] = groups.get(key, 0) | (1 << x)
        return list(groups.values())

    def status(self) -> Status:
        known = self.pos | self.neg
        classes = self.classes()
        if any((c & self.pos) and (c & self.neg) for c in classes):
            return Status.INSUFFICIENT
        if any(not (c & known) for c in classes):
            return Status.UNDETERMINED
        return Status.IDENTIFIED

    def next_question(self) -> int | None:
        known = self.pos | self.neg
        for c in self.classes():
            if not (c & known):
                return (c & -c).bit_length() - 1
        return None

    def answer(self, x: int, is_member: bool) -> None:
        if is_member:
            self.pos |= 1 << x
        else:
            self.neg |= 1 << x

    def propose(self, name: str, mask: int) -> None:
        self.preds.append((name, mask))

    def capacity(self) -> int:
        return 1 << len(self.classes())

    def known(self) -> int:
        return self.pos | self.neg


def _selftest() -> None:
    e = Explorer(4)
    e.propose("a", 0b0011)
    e.answer(0, True)
    e.answer(1, False)
    assert e.status() is Status.INSUFFICIENT
    e.propose("b", 0b0001)
    assert e.status() is not Status.INSUFFICIENT
    assert e.capacity() >= 4


if __name__ == "__main__":
    _selftest()
    print("engine selftest ok")
