"""
Bilingual alignment check: English is authoritative, Chinese is the translation.
This script checks whether the translation has quietly drifted.

It compares five things:
  1. section count and order
  2. environment counts (theorem / definition / figure / table / axis)
  3. the set of citation keys used
  4. whether the key numbers in the paper appear on both sides
  5. whether the mechanization map still matches the Lean development and both papers

It does not judge translation quality; it only catches mechanical problems such as
missing structure or numbers.

Run: python scripts/align-papers.py
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EN = ROOT / "paper" / "en" / "main.tex"
ZH = ROOT / "paper" / "zh" / "main.tex"

ENVS = ["theorem", "definition", "figure", "table", "tabular", "axis"]
KEYS = [
    "2.01", "1.91", "6.14", "2.10", "1.85", "4.28",
    "0.06", "0.07", "0.23", "0.09",
    "27.6", "49.4", "88.9", "162.7", "7.6",
    "3.11", "3.50", "3.60", "57.2", "0.250", "0.955", "1.53", "0.975", "0.890", "0.110", "0.025",
    "$28$", "$88$", "$51$", "$111$", "2.5", "12.6",
    r"2.5\%", r"29.5\%", r"98.7\%", r"0.5\%", r"8\%",
    "r/q", "300/300",
]


def sections(t: str) -> list[str]:
    return re.findall(r"\\section\{([^}]*)\}", t)


def env_counts(t: str) -> dict[str, int]:
    return {e: len(re.findall(r"\\begin\{" + e + r"\}", t)) for e in ENVS}


def cites(t: str) -> set[str]:
    out: set[str] = set()
    for m in re.findall(r"\\cite\{([^}]*)\}", t):
        out |= {k.strip() for k in m.split(",")}
    return out


LEAN = ROOT / "proofs" / "QueryDriven.lean"
MECH = ROOT / "proofs" / "mechanization.md"


def mechanization_names() -> list[str]:
    """Declaration names in the second column of the maintenance table."""
    names: list[str] = []
    for line in MECH.read_text(encoding="utf-8").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 2 or not re.fullmatch(r"Theorem \d+", cells[0]):
            continue
        names += re.findall(r"`([A-Za-z0-9_]+)`", cells[1])
    return names


def main() -> None:
    en = EN.read_text(encoding="utf-8")
    zh = ZH.read_text(encoding="utf-8")
    problems = 0

    se, sz = sections(en), sections(zh)
    print(f"sections: en={len(se)} zh={len(sz)}")
    if len(se) != len(sz):
        problems += 1
        print("  ! section count differs")
    for i, (a, b) in enumerate(zip(se, sz)):
        print(f"  {i}: {a[:36]:38s} | {b[:26]}")

    ee, ez = env_counts(en), env_counts(zh)
    print(f"environments: en={ee}")
    print(f"              zh={ez}")
    for k in ENVS:
        if ee[k] != ez[k]:
            problems += 1
            print(f"  ! {k}: en={ee[k]} zh={ez[k]}")

    ce, cz = cites(en), cites(zh)
    print(f"citations: en={len(ce)} zh={len(cz)}")
    if ce != cz:
        problems += 1
        print(f"  ! citation set differs: {sorted(ce ^ cz)}")

    missing = [k for k in KEYS if (k in en) != (k in zh)]
    print(f"key numbers: {len(KEYS) - len(missing)}/{len(KEYS)} agree on both sides")
    if missing:
        problems += 1
        print(f"  ! present on one side only: {missing}")

    names = mechanization_names()
    if not names:
        problems += 1
        print("  ! no Lean names parsed out of the mechanization map")
    else:
        lean = LEAN.read_text(encoding="utf-8")
        absent = [n for n in names if f"theorem {n}" not in lean and f"def {n}" not in lean]
        print(f"mechanization map: {len(names)} names, {len(absent)} missing from the Lean file")
        if absent:
            problems += 1
            print(f"  ! listed in the map but absent from the Lean file: {absent}")
        claimed = {int(n) for t in (en, zh) for n in re.findall(r"(\d+)\s*Lean", t)}
        if claimed != {len(names)}:
            problems += 1
            print(f"  ! paper claims {sorted(claimed)} declarations, the map lists {len(names)}")

    if problems:
        print(f"\n{problems} structural drift(s)")
        sys.exit(1)
    print("\nstructure and numbers aligned")


if __name__ == "__main__":
    main()
