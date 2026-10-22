"""Keep the engine's dependency direction pointing one way.

Three tiers, from the inside out:

  kernel      types and pure decisions: no I/O, no interfaces, no policy
  runtime     operators, controller, records: may use kernel, never interfaces
  interface   CLI, MCP, metrics report: may use anything

An import may only point inward. A file with no tier is an error, so adding a module forces the
author to say where it belongs instead of defaulting into the kernel.

This is a lint over source text, not a type-system guarantee; it exists because "we will keep the
layering clean" is a promise that decays, while a red gate does not.
"""
import re
import sys
from pathlib import Path

KERNEL = {
    "lib.rs",        # core types: Mask, Explorer, Definition, Status
    "state.rs",
    "operators.rs",
    "propose.rs",    # pure validation over a request and a response
    "metrics.rs",    # pure functions over files
    "coverage.rs",   # pure table analysis
}
RUNTIME = {
    "suggest.rs",
    "explore.rs",
    "apply.rs",
    "answer.rs",
    "laws.rs",
    "predictions.rs",
    "plan.rs",
    "ledger.rs",
    "portfolio.rs",
    "report.rs",
    "score.rs",
}
INTERFACE = {
    "main.rs",
    "mcp.rs",
}

TIER = {}
for name in KERNEL:
    TIER[name] = "kernel"
for name in RUNTIME:
    TIER[name] = "runtime"
for name in INTERFACE:
    TIER[name] = "interface"

RANK = {"kernel": 0, "runtime": 1, "interface": 2}

USE_RE = re.compile(r"^\s*use\s+crate::(?:\{([^}]*)\}|([A-Za-z_][A-Za-z0-9_]*))")


def modules_used(text):
    names = set()
    for line in text.splitlines():
        m = USE_RE.match(line)
        if not m:
            continue
        if m.group(1):
            for part in m.group(1).split(","):
                part = part.strip()
                if not part:
                    continue
                head = part.split("::")[0].strip()
                if head and head[0].islower():
                    names.add(head)
        elif m.group(2):
            names.add(m.group(2))
    return names


def main():
    root = Path(__file__).resolve().parent.parent / "engine" / "src"
    problems = []
    unclassified = []
    for path in sorted(root.glob("*.rs")):
        tier = TIER.get(path.name)
        if tier is None:
            unclassified.append(path.name)
            continue
        for used in modules_used(path.read_text(encoding="utf-8")):
            used_file = f"{used}.rs"
            used_tier = TIER.get(used_file)
            if used_tier is None:
                # A name that is not a module file is a type from lib.rs; that is the kernel itself.
                if used in {"bit", "Mask", "Explorer", "Status", "Definition", "DefOp",
                            "Predicate", "Vocabulary", "canonical_names"}:
                    continue
                problems.append(f"{path.name}: imports unknown module `{used}`")
                continue
            if RANK[used_tier] > RANK[tier]:
                problems.append(
                    f"{path.name} ({tier}) imports {used} ({used_tier}): an inner layer may not use an outer one"
                )
    if unclassified:
        for name in unclassified:
            problems.append(f"{name}: no tier declared in scripts/check-layers.py")
    if problems:
        for problem in problems:
            print(f"[check-layers] {problem}")
        return 1
    print(f"[check-layers] ok: {len(TIER)} files classified, no outward imports")
    return 0


if __name__ == "__main__":
    sys.exit(main())
