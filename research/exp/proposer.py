"""
Proposer interface (offline version): hand the colliding pairs to the model, take back candidate predicates, and test whether they cut them apart.

Why offline: there is no stable API yet. This script builds the interface and the verification; for now the model side is filled in by the authors
(`responses_*.json`), and switching to a real call later only requires replacing `load_responses`.

Protocol:
  request (JSON, printed for the model to read)
    {"case": ..., "objects": [...], "colliding_pairs": [[x,y],...],
     "existing_predicates": [...], "instruction": "..."}
  response (JSON list)
    [{"name": "...", "rationale": "...", "predicate_py": "lambda case: ..."}, ...]
  where `predicate_py` must evaluate to a bool on a case.

Verification: for each candidate, count how many colliding pairs it covers; a candidate covering 0 is judged useless (the engineering meaning of T2).

Run:
  python proposer.py --case domain_c --emit            # print the request only
  python proposer.py --case domain_c --responses responses_domain_c.json
"""
from __future__ import annotations

import argparse
import itertools
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


# ---------- two selectable cases ----------

def case_domain_c():
    """Domain C: interval merging, hidden invariant = the output is shorter than the input."""
    from domain_c import CASES, merge_intervals, target  # noqa: F401

    objs = list(range(len(CASES)))
    return CASES, objs, target


# ---------- protocol ----------

def build_request(case_name: str, cases, objs, pos, neg, existing) -> dict:
    pairs = [[x, y] for x in sorted(pos) for y in sorted(neg)]
    return {
        "case": case_name,
        "objects": [{"id": i, "input": repr(cases[i])} for i in objs],
        "colliding_pairs": pairs,
        "existing_predicates": existing,
        "instruction": (
            "Propose 2-4 candidate predicates. Requirements: each must separate every pair in colliding_pairs; "
            "give it as an executable expression (predicate_py: lambda case: ...); "
            "add a one-line rationale. Output a JSON list."
        ),
    }


def load_responses(path: Path) -> list[dict]:
    return json.loads(path.read_text(encoding="utf-8"))


SAFE = {
    "any": any, "all": all, "len": len, "sorted": sorted, "max": max, "min": min,
    "range": range, "set": set, "abs": abs, "sum": sum, "enumerate": enumerate,
}
from itertools import combinations  # noqa: E402

SOFT_THRESHOLD = 0.8   # evidence reliability below this: only a soft violation, not a hard block

SAFE["combinations"] = combinations


def evaluate(resp: list[dict], cases, pairs, truth: set[int], ledger: dict[int, float] | None = None) -> list[dict]:
    """Score each candidate.

    Besides "how many colliding pairs it cuts", three harder quantities are computed (borrowed from the blind maximality protocol):
      precision / recall: how far the predicate's extension overlaps the hidden target's extension — a candidate that cuts the colliding pairs but means the wrong thing
                          is exposed here.
      pred hit: how many of the objects the candidate itself claims "I predict positive, but the sample did not provide them" are hits.
      probe: reverse blind maximality — an object where "you say no, but I think it is positive"; as soon as the oracle confirms it, the candidate is proven incomplete.
             This one needs only the oracle, not the hidden extension, so it is a deployable completeness probe.
    """
    out = []
    for r in resp:
        try:
            fn = eval(r["predicate_py"], {"__builtins__": SAFE}, {})  # noqa: S307 - filled in offline
            ext = {i for i, c in enumerate(cases) if fn(c)}
            split = sum(1 for (a, b) in pairs if (a in ext) != (b in ext))
            inter = len(ext & truth)
            prec = inter / len(ext) if ext else 0.0
            rec = inter / len(truth) if truth else 0.0
            preds = r.get("predictions", [])
            hits = sum(1 for p in preds if p in truth)

            probes = r.get("counterexamples", [])
            confirmed = 0
            invalid = 0
            for c in probes:
                if c in ext:
                    invalid += 1          # the probe object is already inside the candidate, so it is not a counterexample
                elif c in truth:
                    confirmed += 1        # the candidate says no, the oracle says yes → the candidate is incomplete
            ledger = ledger or {}
            viol_all = [c for c in ledger if c not in ext]      # exclude already-confirmed positives
            viol = [c for c in viol_all if ledger[c] >= SOFT_THRESHOLD]
            soft = [c for c in viol_all if ledger[c] < SOFT_THRESHOLD]

            # the two roles must be judged separately:
            #   definition — the candidate is taken as a complete definition of S, so it must contain every confirmed positive
            #   cover      — the candidate is only an element of a set cover, so narrow but sound is enough (T7 allows several to be combined)
            # it must actually have given predictions that were confirmed; "no prediction" is not sound (otherwise anything would pass)
            sound = len(preds) > 0 and hits == len(preds)
            definition_ok = (not viol) and sound and prec >= 0.999 and rec >= 0.999
            cover_ok = sound and split > 0

            if viol:
                verdict = "INCOMPLETE(ledger)"
            elif not probes:
                verdict = "UNPROBED"
            elif confirmed:
                verdict = "INCOMPLETE"
            elif invalid == len(probes):
                verdict = "INVALID"
            else:
                verdict = "SURVIVED"      # the probe was rejected by the oracle, so the candidate was not broken this round

            out.append({"name": r["name"], "covers": split, "total": len(pairs),
                        "precision": prec, "recall": rec,
                        "pred_hits": hits, "pred_total": len(preds),
                        "probe_total": len(probes), "probe_confirmed": confirmed,
                        "confirmed_cases": [c for c in probes if c not in ext and c in truth],
                        "ledger_violations": len(viol),
                        "ledger_soft": len(soft),
                        "definition_ok": definition_ok, "cover_ok": cover_ok,
                        "verdict": verdict,
                        "rationale": r.get("rationale", ""), "ok": True})
        except Exception as exc:  # noqa: BLE001
            out.append({"name": r.get("name", "?"), "covers": 0, "total": len(pairs),
                        "precision": 0.0, "recall": 0.0, "pred_hits": 0, "pred_total": 0,
                        "probe_total": 0, "probe_confirmed": 0, "ledger_violations": 0,
                        "ledger_soft": 0,
                        "definition_ok": False, "cover_ok": False,
                        "confirmed_cases": [], "verdict": "ERROR",
                        "rationale": r.get("rationale", ""), "ok": False, "error": str(exc)})
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--case", default="domain_c", choices=["domain_c"])
    ap.add_argument("--emit", action="store_true")
    ap.add_argument("--responses")
    ap.add_argument("--pool-out", help="write the pool file used by qde cover (by default only candidates with P=R=1 are kept)")
    ap.add_argument("--pool-filter", default="offline", choices=["definition", "cover", "offline", "all"],
                    help="offline: use P/R from the hidden extension (evaluation); empirical: trust only the oracle's confirmation of predictions")
    ap.add_argument("--ask-predictions", action="store_true", help="print the per-item confirmation of predictions")
    ap.add_argument("--plan", action="store_true", help="print a requery plan: merge conflicted objects by equivalence class")
    ap.add_argument("--rework-cost", default="auto",
                    help="what one mislabel costs to rework (relative to one query); auto = estimate from the ledger history")
    ap.add_argument("--p-pure", type=float, default=None,
                    help="class-purity prior; if omitted, estimate it from the ledger (Laplace smoothing)")
    ap.add_argument("--round", type=int, default=1, help="for ledger records: which round this is")
    ap.add_argument("--ledger", default=str(HERE / "probe_ledger.json"),
                    help="ledger of confirmed counterexamples: a new candidate must pass it first")
    ap.add_argument("--no-ledger-write", action="store_true", help="read the ledger only, do not write back")
    ap.add_argument("--scenario-out", help="write the qde scenario file (including the cover call)")
    args = ap.parse_args()

    from domain_c import PREDICATES, mask_of
    from engine import Explorer

    cases, objs, target = case_domain_c()
    truth = {i: target(c) for i, c in enumerate(cases)}
    pos = {i for i, v in truth.items() if v}
    neg = {i for i, v in truth.items() if not v}

    # hand the model only the positive/negative pairs "indistinguishable under the existing 6 features": that is the subproblem really to be solved
    ex = Explorer(len(cases))
    for name, fn in PREDICATES.items():
        ex.propose(name, mask_of(fn))
    colliding = []
    for c in ex.classes():
        members = [i for i in range(len(cases)) if c >> i & 1]
        colliding += [[a, b] for a in members if a in pos for b in members if b in neg]
    existing = list(PREDICATES.keys())

    req = build_request(args.case, cases, objs, pos, neg, existing)
    req["colliding_pairs"] = colliding
    req["note"] = f"{len(colliding)} colliding pairs (indistinguishable under the existing features)"
    if args.emit or not args.responses:
        print(json.dumps(req, ensure_ascii=False, indent=2))
        return

    if args.plan:
        from ledger import conflicts as find_conflicts, load as load_ledger2
        lp = Path(args.ledger)
        if not lp.exists():
            print("no ledger")
            return
        conf = find_conflicts(load_ledger2(lp))
        if not conf:
            print("no conflicts — nothing to re-ask")
            return
        # known labels: the high-reliability confirmations in the ledger (positive/negative)
        known_pos, known_neg = set(), set()
        for e in load_ledger2(lp)["entries"]:
            ev = e.get("evidence", {})
            if e.get("status", "active") != "active" or float(ev.get("reliability", 1.0)) < 0.8:
                continue
            if e["case"] in conf:
                continue   # a conflicted object does not itself count as a "known label"
            (known_pos if ev.get("answer", True) else known_neg).add(e["case"])

        cls = ex.classes()

        # estimate p (the class-purity prior) from the ledger: only classes with "at least two confirmed labels", Laplace-smoothed
        lab_total = lab_impure = 0
        for mask in cls:
            members = [j for j in range(len(cases)) if mask >> j & 1]
            kp = [j for j in members if j in known_pos]
            kn = [j for j in members if j in known_neg]
            if len(kp) + len(kn) >= 2:
                lab_total += 1
                lab_impure += 1 if (kp and kn) else 0
        # R: the rework cost of one mislabel. Estimated from "how many entries were actually repaired / how many of them needed repair" in the ledger.
        rep_actions = rep_entries = 0
        for e in load_ledger2(lp)["entries"]:
            acts = [h["action"] for h in e.get("history", [])]
            n_fix = sum(a in ("retract", "amend", "supersede") for a in acts)
            if n_fix or e.get("status") in ("retracted", "superseded"):
                rep_entries += 1
                rep_actions += max(1, n_fix)
        if args.rework_cost == "auto":
            r_used = max(1.0, rep_actions / rep_entries) if rep_entries else 1.0
            src_r = f"estimated from ledger ({rep_actions} repairs / {rep_entries} repaired entries)"
        else:
            r_used = float(args.rework_cost)
            src_r = "given on command line"
        print(f"[plan] R={r_used:.2f}  {src_r}")

        if args.p_pure is None:
            p_used = (lab_impure + 1) / (lab_total + 2) if lab_total else 0.5
            src = f"estimated from ledger ({lab_impure}/{lab_total} impure)"
        else:
            p_used = args.p_pure
            src = "given on command line"
        print(f"[plan] p(pure)={p_used:.2f}  {src}")

        groups: dict[int, list[int]] = {}
        for case in sorted(conf):
            for i, mask in enumerate(cls):
                if (mask >> case) & 1:
                    groups.setdefault(i, []).append(case)
                    break

        print(f"conflicted objects {sorted(conf)}; {len(groups)} classes:")
        print(f"{'class':>6s} {'size':>5s} {'known +/-':>10s} {'purity':>14s} {'conflicted':>12s} {'asks':>5s}")
        total = 0
        for i, cases_in in groups.items():
            members = [j for j in range(len(cases)) if cls[i] >> j & 1]
            kp = [j for j in members if j in known_pos]
            kn = [j for j in members if j in known_neg]
            if kp and kn:
                purity, asks = "impure(known)", len(cases_in)
            elif len(members) == 1:
                purity, asks = "pure?(single)", 1
            else:
                # unknown purity: ask one representative and gamble, or ask all conflicted members?
                # ask 1: 1 + (1-p)·(c-1)·R (if the guess is wrong the other members must be relabeled, each costing R)
                # ask all: c
                c_ = len(cases_in)
                cost_one = 1 + (1 - p_used) * (c_ - 1) * r_used
                cost_all = c_
                if c_ == 1 or cost_one <= cost_all:
                    purity, asks = "unknown", 1
                else:
                    purity, asks = "unknown(buy-safe)", c_
            total += asks
            extra = ""
            if purity.startswith("unknown") and len(cases_in) > 1:
                c_ = len(cases_in)
                extra = (f"   cost(1)={1 + (1-p_used)*(c_-1)*r_used:.2f}"
                         f" cost(all)={c_:.2f}")
            print(f"{i:>6d} {len(members):>5d} {f'{len(kp)}/{len(kn)}':>10s} {purity:>16s} "
                  f"{str(cases_in):>12s} {asks:>5d}{extra}")
        print(f"\ntotal {total} questions; asking one by one needs {len(conf)}")
        if any(len([j for j in range(len(cases)) if cls[i] >> j & 1]) > 1
               and not ([j for j in range(len(cases)) if cls[i] >> j & 1 and j in known_pos] and
                        [j for j in range(len(cases)) if cls[i] >> j & 1 and j in known_neg])
               for i in groups):
            print("note: for an unknown class, expected cost decides between \"ask 1\" and \"ask every conflicted member\"; "
                  "unknown(buy-safe) means a mislabel is too costly to gamble on.")
        return

    resp = load_responses(Path(args.responses))
    ledger_path = Path(args.ledger)
    from ledger import active_cases, load as load_ledger, next_id, now, save as save_ledger

    ledger_rel: dict[int, float] = {}
    conflicted: set[int] = set()
    if ledger_path.exists():
        from ledger import conflicts as find_conflicts, directions
        data = load_ledger(ledger_path)
        conflicted = set(find_conflicts(data))
        # merge only entries with a consistent direction that are not conflicted
        ledger_rel = {c: d["pos"] for c, d in directions(data).items()
                      if c not in conflicted and d["pos"] > 0}
    print(f"[ledger] conflicts: {sorted(conflicted) if conflicted else 'none'}")
    ledger: set[int] = set(ledger_rel)
    print(f"[ledger] {len(ledger)} confirmed counterexamples")
    rows = evaluate(resp, cases, req["colliding_pairs"], pos, ledger_rel)

    if args.pool_out:
        # two filtering paths:
        #   offline (default): compute P/R from the hidden extension — evaluation only, not computable at deployment time.
        #   empirical: treat the candidate's self-reported predictions as queries to the oracle and keep only candidates
        #              with "all predictions correct and at least one pair cut". This is the deployment path: it depends on the oracle's answers, not on the hidden target.
        if args.pool_filter in ("definition",):
            kept = [r for r in rows if r["ok"] and r["definition_ok"]]
        elif args.pool_filter in ("cover", "empirical"):
            if args.ask_predictions:
                print(f"[ask] asking the oracle about the candidates' predictions ("
                      f"{sum(r['pred_total'] for r in rows)} in total)")
                for r in rows:
                    if r["pred_total"]:
                        print(f"  {r['name']:26s} confirmed {r['pred_hits']}/{r['pred_total']}")
            kept = [r for r in rows
                    if r["ok"] and r["pred_total"] > 0
                    and r["pred_hits"] == r["pred_total"] and r["covers"] > 0]
        elif args.pool_filter == "offline":
            kept = [r for r in rows if r["ok"] and r["precision"] >= 0.999 and r["recall"] >= 0.999]
        else:
            kept = [r for r in rows if r["ok"]]

        lines = []
        for r in kept:
            fn = eval(next(x["predicate_py"] for x in resp if x["name"] == r["name"]),
                      {"__builtins__": SAFE}, {})
            ids = [str(i) for i, c in enumerate(cases) if fn(c)]
            lines.append(f"{r['name']} " + " ".join(ids))
        Path(args.pool_out).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"[pool] kept {len(kept)}/{len(rows)} ({args.pool_filter}) → {args.pool_out}")
        if len(kept) < len(rows):
            dropped = [r["name"] for r in rows if r not in kept]
            print(f"[pool] filtered: {', '.join(dropped)}")

    if args.scenario_out:
        from domain_c import PREDICATES as P6, mask_of as m6
        n = len(cases)
        lines = [f"objects {n}"]
        for name, fn in P6.items():
            ids = [str(i) for i in range(n) if m6(fn) >> i & 1]
            lines.append(f"add {name} " + " ".join(ids))
        # the seeds must be given 3 votes, otherwise the "weak-vote blocking" rule stops them (which is exactly the designed behavior)
        seeds = req["colliding_pairs"][0]
        lines.append(f"vote {seeds[0]} 3 0")
        lines.append(f"vote {seeds[1]} 0 3")
        lines.append("need")
        if args.pool_out:
            lines.append(f"cover {args.pool_out}")
        Path(args.scenario_out).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"[scenario] → {args.scenario_out}")
    print(f"{'name':26s} {'cover':>9s} {'P':>5s} {'R':>5s} {'preds':>7s}  rationale")
    for r in rows:
        err = (' ' + r.get("error", ""))[:46] if not r["ok"] else ""
        pred = f"{r['pred_hits']}/{r['pred_total']}" if r["pred_total"] else "—"
        print(f"{r['name']:26s} {r['covers']:>4d}/{r['total']:<4d} "
              f"{r['precision']:5.2f} {r['recall']:5.2f} {pred:>7s}  {r['rationale'][:30]}{err}")

    print(f"\n{'name':26s} {'probes':>6s} {'conf':>5s} {'hard':>4s} {'soft':>4s} {'def':>4s} {'cover':>6s}  verdict")
    for r in rows:
        print(f"{r['name']:26s} {r['probe_total']:>6d} {r['probe_confirmed']:>5d} "
              f"{r['ledger_violations']:>4d} {r['ledger_soft']:>4d} "
              f"{'Y' if r['definition_ok'] else 'n':>4s} {'Y' if r['cover_ok'] else 'n':>6s}  {r['verdict']}")

    new_cases = [(c, r["name"]) for r in rows for c in r.get("confirmed_cases", []) if c not in ledger]
    if new_cases and not args.no_ledger_write:
        data = load_ledger(ledger_path)
        for case, by in new_cases:
            e = {"id": next_id(data), "case": case, "by": by, "round": args.round,
                 "kind": "positive", "status": "active", "reason": "", "created": now(), "history": [],
                 "evidence": {"source": "domain_c: merge_intervals(output) < input",
                              "answer": True, "votes": "", "recorded": now()}}
            e["history"].append({"at": now(), "action": "add", "note": f"found by {by}"})
            data["entries"].append(e)
        save_ledger(ledger_path, data)
        print(f"[ledger] +{len(new_cases)} entries {[c for c, _ in new_cases]} → {len(active_cases(data))} active")


if __name__ == "__main__":
    main()





























