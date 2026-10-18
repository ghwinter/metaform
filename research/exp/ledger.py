"""
Counterexample ledger: a knowledge base that can **grow, shrink, and change**, not a monotone set.

Why changes are needed: a counterexample may come from a noisy oracle, may name the wrong object, or may later turn out to hold only for one kind of candidate.
A monotone set would permanently freeze one mistake, and every later candidate would be killed by it.

Design:
  - every record has id, case, by (who broke it), round, kind, status, reason, history.
  - status = active | retracted; only active records take part in decisions.
  - delete = retract (kept in history with a reason); modify = amend (written into history).
  - every change carries a timestamp, so any decision can be traced back to the ledger state at that time.

Command line:
  python ledger.py list [--all]
  python ledger.py add --case 4 --by has_repeated_endpoint --round 1
  python ledger.py retract L2 --reason "the oracle answered wrong at the time"
  python ledger.py amend L3 --case 5 --reason "named the wrong object"
  python ledger.py history
"""
from __future__ import annotations

import argparse
import json
import pathlib
from datetime import datetime

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT = HERE / "probe_ledger.json"


def now() -> str:
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def load(path: pathlib.Path) -> dict:
    if not path.exists():
        return {"version": 2, "entries": []}
    raw = json.loads(path.read_text(encoding="utf-8"))
    if "entries" in raw:
        # old entries carry no evidence: fill one in from the information available, without lying (source is set to by)
        for e in raw["entries"]:
            e.setdefault("evidence", {
                "source": e.get("by", "migrated"),
                "answer": e.get("kind", "positive") == "positive",
                "votes": "",
                "reliability": 1.0,
                "recorded": e.get("created", now()),
            })
        return raw
    # migration: v1's {"cases": [3,4,6]} or [{"id":...}]
    entries = []
    for i, x in enumerate(raw.get("cases", []), 1):
        if isinstance(x, int):
            entries.append({"id": f"L{i}", "case": x, "by": "migrated", "round": 0,
                            "kind": "positive", "status": "active", "reason": "",
                            "created": now(), "history": []})
        else:
            entries.append({"id": f"L{i}", "case": x.get("id"), "by": x.get("by", "migrated"),
                            "round": x.get("round", 0), "kind": x.get("kind", "positive"),
                            "status": "active", "reason": "", "created": now(), "history": [],
                            "evidence": {"source": x.get("by", "migrated"), "answer": True,
                                         "votes": "", "reliability": 1.0, "recorded": now()}})
    return {"version": 2, "entries": entries}


def save(path: pathlib.Path, data: dict) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def active_cases(data: dict) -> set[int]:
    return {e["case"] for e in data["entries"] if e.get("status", "active") == "active"}


def stale_cases(data: dict) -> set[int]:
    """Entries suspended, waiting to be re-confirmed under the new target."""
    return {e["case"] for e in data["entries"] if e.get("status") == "stale"}


def retarget(data: dict, to: str, reason: str) -> int:
    """Switch the target concept: suspend all old entries (stale), neither deleting them nor using them in decisions, and wait to re-confirm them one by one."""
    n = 0
    for e in data["entries"]:
        if e.get("status", "active") == "active":
            e["status"] = "stale"
            log(e, "stale", f"retarget {data.get('target','S0')} -> {to}: {reason}")
            n += 1
    data["target"] = to
    return n


def next_id(data: dict) -> str:
    n = len(data["entries"]) + 1
    while any(e["id"] == f"L{n}" for e in data["entries"]):
        n += 1
    return f"L{n}"


def log(e: dict, action: str, note: str) -> None:
    e.setdefault("history", []).append({"at": now(), "action": action, "note": note})


def cmd_list(args) -> None:
    data = load(args.ledger)
    print(f"{'id':5s} {'case':>5s} {'status':>10s} {'by':22s} round  evidence  reason")
    for e in data["entries"]:
        if e["status"] != "active" and not args.all:
            continue
        ev = e.get("evidence", {})
        evs = f"{ev.get('answer', '?')}/{ev.get('source', '?')[:10]}/r{ev.get('reliability', 1.0):.2f}"
        print(f"{e['id']:5s} {e['case']:>5d} {e['status']:>10s} {e['by'][:22]:22s} "
              f"{e.get('round', 0):>4d}  {evs:22s}  {e.get('reason', '')}")
    print(f"\nactive cases: {sorted(active_cases(data))}")


def cmd_add(args) -> None:
    data = load(args.ledger)
    dup = [e["id"] for e in data["entries"]
           if e["case"] == args.case and e.get("status", "active") == "active"]
    if dup:
        print(f"注意：case {args.case} 已有 active 条目 {dup}，将作为独立证据并存")
    e = {"id": next_id(data), "case": args.case, "by": args.by, "round": args.round,
         "kind": args.kind, "status": "active", "reason": args.reason or "",
         "created": now(), "history": [],
         "evidence": {"source": args.source, "answer": args.answer == "true",
                      "votes": args.votes, "reliability": args.reliability,
                      "recorded": now()}}
    log(e, "add", args.reason or "")
    data["entries"].append(e)
    save(args.ledger, data)
    print(f"+ {e['id']} case {args.case}")


def cmd_retract(args) -> None:
    data = load(args.ledger)
    for e in data["entries"]:
        if e["id"] == args.id:
            e["status"] = "retracted"
            e["reason"] = args.reason
            log(e, "retract", args.reason)
            save(args.ledger, data)
            print(f"- {e['id']} (case {e['case']}) retracted: {args.reason}")
            return
    raise SystemExit(f"no such id: {args.id}")


def cmd_amend(args) -> None:
    data = load(args.ledger)
    for e in data["entries"]:
        if e["id"] == args.id:
            before = (e["case"], e["kind"])
            if args.case is not None:
                e["case"] = args.case
            if args.kind:
                e["kind"] = args.kind
            if args.by:
                e["by"] = args.by
            if args.reliability is not None:
                e["evidence"]["reliability"] = args.reliability
            log(e, "amend", f"{before} -> {(e['case'], e['kind'])}; {args.reason}")
            e["reason"] = args.reason
            save(args.ledger, data)
            print(f"~ {e['id']} amended to case {e['case']} ({e['kind']}): {args.reason}")
            return
    raise SystemExit(f"no such id: {args.id}")


def combined(entries: list[dict]) -> float:
    """Noisy-OR of independent evidence: 1 - Π(1 - r_i)."""
    prod = 1.0
    for e in entries:
        prod *= (1.0 - float(e.get("evidence", {}).get("reliability", 1.0)))
    return 1.0 - prod


def directions(data: dict) -> dict[int, dict[str, float]]:
    """Merge reliability by direction: {case: {"pos": r, "neg": r}}."""
    out: dict[int, dict[str, float]] = {}
    for e in data["entries"]:
        if e.get("status", "active") != "active":
            continue
        r = float(e.get("evidence", {}).get("reliability", 1.0))
        d = "pos" if e.get("evidence", {}).get("answer", True) else "neg"
        cur = out.setdefault(e["case"], {"pos": 0.0, "neg": 0.0})
        cur[d] = 1.0 - (1.0 - cur[d]) * (1.0 - r)
    return out


def conflicts(data: dict, threshold: float = 0.5, ratio: float = 0.7) -> dict[int, dict[str, float]]:
    """The same object has positive and negative evidence at once, and the two sides are close → conflict: the truth is undetermined.

    If one side overwhelms the other (min/max < ratio), it is not a conflict and the stronger side speaks —
    overwhelming evidence should not be cancelled by a weak opposing vote.
    """
    out = {}
    for c, d in directions(data).items():
        lo, hi = d["pos"], d["neg"]
        if lo > hi:
            lo, hi = hi, lo
        if lo >= threshold and hi > 0 and lo >= ratio * hi:
            out[c] = d
    return out


def votes_needed(r_query: float, target: float = 0.95) -> int:
    """How many more independent confirmations are needed for one side to reach target (assuming reliability r_query each)."""
    if r_query <= 0 or r_query >= 1:
        return 0
    n, acc = 0, 0.0
    while acc < target and n < 50:
        n += 1
        acc = 1.0 - (1.0 - acc) * (1.0 - r_query)
    return n


def cmd_conflicts(args) -> None:
    data = load(args.ledger)
    conf = conflicts(data, args.threshold, args.ratio)
    if not conf:
        print("no conflicts")
        return

    # work queue: the smaller the gap, the higher the priority; also report "how many more independent confirmations"
    queue = []
    for case, d in conf.items():
        gap = abs(d["pos"] - d["neg"])
        queue.append({"case": case, "pos": round(d["pos"], 3), "neg": round(d["neg"], 3),
                      "gap": round(gap, 3),
                      "votes": votes_needed(args.r_query)})
    queue.sort(key=lambda x: x["gap"])

    if args.json:
        print(json.dumps({"requery": queue, "r_query": args.r_query}, ensure_ascii=False, indent=2))
        return

    print(f"重问队列（按差距升序，r_query={args.r_query}）：")
    print(f"{'case':>5s} {'pos':>6s} {'neg':>6s} {'gap':>6s} {'再问':>5s}")
    for q in queue:
        print(f"{q['case']:>5d} {q['pos']:6.2f} {q['neg']:6.2f} {q['gap']:6.2f} {q['votes']:>5d}")
    print(f"\n{len(queue)} 个对象待重问；差距小的先问")


def cmd_corroborate(args) -> None:
    data = load(args.ledger)
    groups: dict[int, list[dict]] = {}
    for e in data["entries"]:
        if e.get("status", "active") == "active":
            groups.setdefault(e["case"], []).append(e)

    for case, es in sorted(groups.items()):
        r = combined(es)
        mark = "OK" if r >= args.threshold else "weak"
        ids = ", ".join(e["id"] for e in es)
        print(f"case {case:>3d}: {len(es)} 条 [{ids}]  合并可信度 {r:.2f}  {mark}")
        if args.apply and r >= args.threshold and len(es) > 1:
            new = {"id": next_id(data), "case": case, "by": "corroborated",
                   "round": max(e.get("round", 0) for e in es), "kind": "positive",
                   "status": "active", "reason": f"由 {ids} 交叉确认", "created": now(),
                   "history": [{"at": now(), "action": "corroborate",
                                "note": f"{ids} → r={r:.2f}"}],
                   "evidence": {"source": "corroborated(" + ids + ")", "answer": True,
                                "votes": "", "reliability": r, "recorded": now(),
                                "derived_from": [e["id"] for e in es]}}
            for e in es:
                e["status"] = "superseded"
                log(e, "supersede", f"合并进 {new['id']}（r={r:.2f}）")
            data["entries"].append(new)
            print(f"  + {new['id']} 生效（原条目转为 superseded）")
    if args.apply:
        save(args.ledger, data)


def cmd_normalize(args) -> None:
    data = load(args.ledger)
    save(args.ledger, data)
    n = sum(1 for e in data["entries"] if "evidence" in e)
    print(f"normalized: {n} entries now carry evidence")


def cmd_show(args) -> None:
    data = load(args.ledger)
    for e in data["entries"]:
        if e["id"] == args.id:
            print(json.dumps(e, ensure_ascii=False, indent=2))
            return
    raise SystemExit(f"no such id: {args.id}")


def cmd_retarget(args) -> None:
    data = load(args.ledger)
    n = retarget(data, args.to, args.reason)
    save(args.ledger, data)
    print(f"~ retarget -> {args.to}: {n} 条挂起（stale），等待重新确认")


def cmd_revalidate(args) -> None:
    data = load(args.ledger)
    for e in data["entries"]:
        if e["id"] == args.id:
            if e.get("status") != "stale":
                raise SystemExit(f"{args.id} 不是 stale（当前 {e.get('status')}）")
            e["status"] = "active"
            log(e, "revalidate", f"在目标 {data.get('target','S0')} 下重新确认；{args.reason}")
            save(args.ledger, data)
            print(f"+ {e['id']} (case {e['case']}) 重新生效")
            return
    raise SystemExit(f"no such id: {args.id}")


def cmd_history(args) -> None:
    data = load(args.ledger)
    for e in data["entries"]:
        print(f"{e['id']} case {e['case']} [{e['status']}]")
        for h in e.get("history", []):
            print(f"    {h['at']}  {h['action']:8s} {h['note']}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--ledger", type=pathlib.Path, default=DEFAULT)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("list"); p.add_argument("--all", action="store_true"); p.set_defaults(fn=cmd_list)
    p = sub.add_parser("add")
    p.add_argument("--case", type=int, required=True)
    p.add_argument("--by", default="manual")
    p.add_argument("--round", type=int, default=0)
    p.add_argument("--kind", default="positive", choices=["positive", "negative"])
    p.add_argument("--reason", default="")
    p.add_argument("--source", default="manual", help="这条判断凭什么：oracle / 测试 / 人工")
    p.add_argument("--answer", default="true", choices=["true", "false"])
    p.add_argument("--votes", default="", help='票数，如 "3 0"')
    p.add_argument("--reliability", type=float, default=1.0,
                   help="这条证据有多可信：单元测试/Lean=1.0，模型按 1-ε")
    p.set_defaults(fn=cmd_add)
    p = sub.add_parser("retract")
    p.add_argument("id"); p.add_argument("--reason", required=True); p.set_defaults(fn=cmd_retract)
    p = sub.add_parser("amend")
    p.add_argument("id"); p.add_argument("--case", type=int); p.add_argument("--kind",
        choices=["positive", "negative"]); p.add_argument("--by")
    p.add_argument("--reliability", type=float); p.add_argument("--reason", required=True)
    p.set_defaults(fn=cmd_amend)
    p = sub.add_parser("show"); p.add_argument("id"); p.set_defaults(fn=cmd_show)
    p = sub.add_parser("normalize"); p.set_defaults(fn=cmd_normalize)
    p = sub.add_parser("conflicts")
    p.add_argument("--threshold", type=float, default=0.5)
    p.add_argument("--ratio", type=float, default=0.7, help="min/max 低于此值就不算冲突")
    p.add_argument("--json", action="store_true", help="输出机器可读的重问队列")
    p.add_argument("--r-query", type=float, default=0.6, help="假设每次重问的可信度")
    p.set_defaults(fn=cmd_conflicts)
    p = sub.add_parser("corroborate")
    p.add_argument("--threshold", type=float, default=0.8)
    p.add_argument("--apply", action="store_true", help="达到阈值就合并成一条")
    p.set_defaults(fn=cmd_corroborate)
    p = sub.add_parser("retarget")
    p.add_argument("--to", required=True, help="新的目标概念名，例如 S1")
    p.add_argument("--reason", required=True)
    p.set_defaults(fn=cmd_retarget)
    p = sub.add_parser("revalidate")
    p.add_argument("id"); p.add_argument("--reason", default="")
    p.set_defaults(fn=cmd_revalidate)
    p = sub.add_parser("history"); p.set_defaults(fn=cmd_history)

    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()









