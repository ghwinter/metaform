"""
提议器接口（离线版）：把撞车对交给模型，收回候选谓词，验它们能不能劈开。

为什么要离线：现在没有稳定的 API。这个脚本把接口与验证做出来，模型一侧先由作者代填
（`responses_*.json`），日后换成真调用只需替换 `load_responses`。

协议：
  请求（JSON，打印出来给模型看）
    {"case": ..., "objects": [...], "colliding_pairs": [[x,y],...],
     "existing_predicates": [...], "instruction": "..."}
  响应（JSON 列表）
    [{"name": "...", "rationale": "...", "predicate_py": "lambda case: ..."}, ...]
  其中 `predicate_py` 必须能在 case 上求值成 bool。

验证：对每个候选，算出它覆盖了几对撞车；覆盖 0 的判为无用（T2 的工程含义）。

运行：
  python proposer.py --case domain_c --emit            # 只打印请求
  python proposer.py --case domain_c --responses responses_domain_c.json
"""
from __future__ import annotations

import argparse
import itertools
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent


# ---------- 两个可选 case ----------

def case_domain_c():
    """域 C：区间合并，隐藏不变量 = 输出比输入短。"""
    from domain_c import CASES, merge_intervals, target  # noqa: F401

    objs = list(range(len(CASES)))
    return CASES, objs, target


# ---------- 协议 ----------

def build_request(case_name: str, cases, objs, pos, neg, existing) -> dict:
    pairs = [[x, y] for x in sorted(pos) for y in sorted(neg)]
    return {
        "case": case_name,
        "objects": [{"id": i, "input": repr(cases[i])} for i in objs],
        "colliding_pairs": pairs,
        "existing_predicates": existing,
        "instruction": (
            "提出 2-4 个候选谓词，要求：能分开 colliding_pairs 里的每一对；"
            "以可执行表达式的形式给出（predicate_py: lambda case: ...）；"
            "并给一句 rationale。输出 JSON 列表。"
        ),
    }


def load_responses(path: Path) -> list[dict]:
    return json.loads(path.read_text(encoding="utf-8"))


SAFE = {
    "any": any, "all": all, "len": len, "sorted": sorted, "max": max, "min": min,
    "range": range, "set": set, "abs": abs, "sum": sum, "enumerate": enumerate,
}
from itertools import combinations  # noqa: E402

SOFT_THRESHOLD = 0.8   # 证据可信度低于此值：只算软违规，不硬拦

SAFE["combinations"] = combinations


def evaluate(resp: list[dict], cases, pairs, truth: set[int], ledger: dict[int, float] | None = None) -> list[dict]:
    """给每个候选打分。

    除了"劈开几对撞车"，还算三个更硬的量（借自盲极大协议）：
      precision / recall：谓词的外延与隐藏目标的外延有多重合——只劈开撞车对但含义不对的候选，
                         在这里会露馅。
      pred hit：候选自己报的"我预测为正、但样本里没给"的对象命中几个。
      probe：反向盲极大——"你说否、但我认为为正"的对象，oracle 一确认，该候选就被证明不完整。
             这条只需要 oracle，不需要隐藏外延，是可以部署的完整性探针。
    """
    out = []
    for r in resp:
        try:
            fn = eval(r["predicate_py"], {"__builtins__": SAFE}, {})  # noqa: S307 - 离线自填
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
                    invalid += 1          # 探针对象本来就在候选里，不是反例
                elif c in truth:
                    confirmed += 1        # 候选说否，oracle 说是 → 候选不完整
            ledger = ledger or {}
            viol_all = [c for c in ledger if c not in ext]      # 排除已确认正例
            viol = [c for c in viol_all if ledger[c] >= SOFT_THRESHOLD]
            soft = [c for c in viol_all if ledger[c] < SOFT_THRESHOLD]

            # 两个角色要分开判：
            #   definition —— 候选被当成 S 的完整定义，必须包含全部已确认正例
            #   cover      —— 候选只作为集合覆盖的元素，窄但 sound 就够（T7 允许多个拼起来）
            # 必须真的给过预测并被确认；"没预测" 不等于 sound（否则谁都能过）
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
                verdict = "SURVIVED"      # 探针被 oracle 否掉，候选在这一轮没被击破

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
    ap.add_argument("--pool-out", help="写出 qde cover 用的池文件（默认只留 P=R=1 的候选）")
    ap.add_argument("--pool-filter", default="offline", choices=["definition", "cover", "offline", "all"],
                    help="offline: 用隐藏外延的 P/R（评估）；empirical: 只信 oracle 对 predictions 的确认")
    ap.add_argument("--ask-predictions", action="store_true", help="打印对 predictions 的逐条确认")
    ap.add_argument("--plan", action="store_true", help="给出重问计划：按等价类合并冲突对象")
    ap.add_argument("--rework-cost", default="auto",
                    help="一个错标要花多少代价返工（相对一次询问）；auto = 从台账历史估计")
    ap.add_argument("--p-pure", type=float, default=None,
                    help="类纯度先验；不给就从台账估计（Laplace 平滑）")
    ap.add_argument("--round", type=int, default=1, help="台账记录用：这是第几轮")
    ap.add_argument("--ledger", default=str(HERE / "probe_ledger.json"),
                    help="已确认反例的台账：新候选必须先过它")
    ap.add_argument("--no-ledger-write", action="store_true", help="只读台账，不写回")
    ap.add_argument("--scenario-out", help="写出 qde 场景文件（含 cover 调用）")
    args = ap.parse_args()

    from domain_c import PREDICATES, mask_of
    from engine import Explorer

    cases, objs, target = case_domain_c()
    truth = {i: target(c) for i, c in enumerate(cases)}
    pos = {i for i, v in truth.items() if v}
    neg = {i for i, v in truth.items() if not v}

    # 只把"在现有 6 个特征下不可分辨"的正反例对交给模型：这才是真正要解的子问题
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
    req["note"] = f"共 {len(colliding)} 对撞车（在现有特征下不可分辨）"
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
        # 已知标签：台账里高可信度的确认结果（正/负）
        known_pos, known_neg = set(), set()
        for e in load_ledger2(lp)["entries"]:
            ev = e.get("evidence", {})
            if e.get("status", "active") != "active" or float(ev.get("reliability", 1.0)) < 0.8:
                continue
            if e["case"] in conf:
                continue   # 冲突对象本身不算"已知标签"
            (known_pos if ev.get("answer", True) else known_neg).add(e["case"])

        cls = ex.classes()

        # 从台账估计 p（类纯的先验）：只看"至少有两个已确认标签"的类，Laplace 平滑
        lab_total = lab_impure = 0
        for mask in cls:
            members = [j for j in range(len(cases)) if mask >> j & 1]
            kp = [j for j in members if j in known_pos]
            kn = [j for j in members if j in known_neg]
            if len(kp) + len(kn) >= 2:
                lab_total += 1
                lab_impure += 1 if (kp and kn) else 0
        # R：一次错标的返工代价。用台账里"实际修过几条 / 其中需要修几条"来估。
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

        print(f"冲突对象 {sorted(conf)}；{len(groups)} 个类：")
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
                # 未知纯度：问一个代表赌一把，还是问完所有冲突成员？
                # 问 1 个：1 + (1-p)·(c-1)·R（猜错时其余成员要重标，每个代价 R）
                # 问全部：c
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
        print(f"\n合计 {total} 次询问；逐个问需 {len(conf)} 次")
        if any(len([j for j in range(len(cases)) if cls[i] >> j & 1]) > 1
               and not ([j for j in range(len(cases)) if cls[i] >> j & 1 and j in known_pos] and
                        [j for j in range(len(cases)) if cls[i] >> j & 1 and j in known_neg])
               for i in groups):
            print("注意：unknown 类按期望成本在「问 1 个」与「问全部冲突成员」之间选；"
                  "unknown(buy-safe) 表示错标代价高到不值得赌。")
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
        # 只合并方向一致、且不处于冲突的条目
        ledger_rel = {c: d["pos"] for c, d in directions(data).items()
                      if c not in conflicted and d["pos"] > 0}
    print(f"[ledger] conflicts: {sorted(conflicted) if conflicted else 'none'}")
    ledger: set[int] = set(ledger_rel)
    print(f"[ledger] {len(ledger)} 个已确认反例")
    rows = evaluate(resp, cases, req["colliding_pairs"], pos, ledger_rel)

    if args.pool_out:
        # 两条过滤路径：
        #   offline（默认）：用隐藏外延算 P/R —— 只用于评估，部署时算不出来。
        #   empirical：把候选自报的 predictions 当查询去问 oracle，只保留"预测全对且至少劈开一对"的
        #              候选。这是部署路径：不依赖隐藏目标，只依赖 oracle 的回答。
        if args.pool_filter in ("definition",):
            kept = [r for r in rows if r["ok"] and r["definition_ok"]]
        elif args.pool_filter in ("cover", "empirical"):
            if args.ask_predictions:
                print(f"[ask] 向 oracle 询问候选的预测（共 "
                      f"{sum(r['pred_total'] for r in rows)} 条）")
                for r in rows:
                    if r["pred_total"]:
                        print(f"  {r['name']:26s} 确认 {r['pred_hits']}/{r['pred_total']}")
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
        # 种子要给到 θ=3 票，否则会被"弱票阻塞"规则挡住（这正是设计要的行为）
        seeds = req["colliding_pairs"][0]
        lines.append(f"vote {seeds[0]} 3 0")
        lines.append(f"vote {seeds[1]} 0 3")
        lines.append("need")
        if args.pool_out:
            lines.append(f"cover {args.pool_out}")
        Path(args.scenario_out).write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"[scenario] → {args.scenario_out}")
    print(f"{'name':26s} {'覆盖':>9s} {'P':>5s} {'R':>5s} {'预测':>7s}  rationale")
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





























