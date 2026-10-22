"""A local model as the proposer: q0 and q1, the two numbers the project has never had.

The claim under test is narrow and it is the only one still unverified:

    q0  the share of a proposer's first answers that the kernel reports as `splits: 0`
    q1  among those, the share that is still `splits: 0` after it is told why

`q0 - q1` is the kernel's contribution on a small local model. If it is zero, the kernel is
a filter with nothing to filter and F3 has a counterexample; if it is positive, this is the
first positive evidence the project has produced. Nothing here decides which — it only measures.

Why a small model and not a large one: the kernel can only help a proposer that fails. A proposer
near the ceiling makes every arm look the same (`RESULTS.md` section 20: baseline 6/6 without the
kernel, 5/6 with it), and asking one model thirty times yields thirty correlated samples, so the
error bars would be fiction.

The pipeline follows `llm_arms.py`: nothing here knows how to talk to a model by default. It writes
prompts, something answers them, and `score` asks the real `metaform propose` binary what the answers
were worth. `answer --endpoint` fills them from an OpenAI-compatible server (llama.cpp on :8080) and
`answer --mock` fills them from a synthetic recombination-biased proposer, so the whole path can be
tested before any weights are downloaded.

Usage:
  python local_proposer.py instances --n 30 --seed 7 --out tmp/q/
  python local_proposer.py prompt    --instances tmp/q/instances.json --round 1 --out tmp/q/p1.json
  python local_proposer.py prompt    --instances tmp/q/instances.json --round 2 \
                                     --first tmp/q/a1.json --out tmp/q/p2.json
  python local_proposer.py answer    --prompts tmp/q/p1.json --mock --out tmp/q/a1.json
  python local_proposer.py answer    --prompts tmp/q/p1.json --endpoint http://127.0.0.1:8080 \
                                     --model local --out tmp/q/a1.json
  python local_proposer.py score     --instances tmp/q/instances.json --first tmp/q/a1.json \
                                     --second tmp/q/a2.json

Every scenario is generated from `--seed`, so the instance set is reproducible and the only thing
that varies between runs is the model.
"""
from __future__ import annotations

import argparse
import json
import random
import subprocess
import sys
import urllib.request
from pathlib import Path

PROTOCOL = "metaform.propose/1"


def engine_path(explicit: str | None) -> Path:
    if explicit:
        return Path(explicit)
    root = Path(__file__).resolve().parent.parent
    for candidate in (root / "target" / "debug" / "metaform.exe",
                      root / "target" / "debug" / "metaform"):
        if candidate.exists():
            return candidate
    raise SystemExit("no metaform binary; build it or pass --engine")


def ids(mask: int, n: int) -> list[int]:
    return [x for x in range(n) if mask >> x & 1]


def mask_of(ids_list) -> int:
    m = 0
    for x in ids_list:
        m |= 1 << int(x)
    return m


def classes(n: int, vocab: list[tuple[str, int]]) -> list[int]:
    """Equivalence classes of the objects under the vocabulary, in first-object order."""
    seen: list[tuple] = []
    out: list[int] = []
    for x in range(n):
        sig = tuple(m >> x & 1 for _, m in vocab)
        if sig in seen:
            out[seen.index(sig)] |= 1 << x
        else:
            seen.append(sig)
            out.append(1 << x)
    return out


def definable(cs: list[int], m: int) -> bool:
    return all((m & c) in (0, c) for c in cs)


def splits(cs: list[int], pos: int, neg: int, ext: int) -> int:
    total = 0
    for c in cs:
        hits = [x for x in range(64) if c >> x & 1]
        for a in hits:
            for b in hits:
                if (pos >> a & 1) and (neg >> b & 1):
                    if ((ext >> a) & 1) != ((ext >> b) & 1):
                        total += 1
    return total


def scene(rng: random.Random, n: int, k: int) -> dict:
    """One scenario: a vocabulary, an evidence split, and objects held back for the bet.

    A colliding pair is planted rather than hoped for. Without one, `splits` is 0 for every
    answer -- which would be read as a proposer failure when it is a bad instance.
    """
    for _ in range(400):
        vocab = []
        for i in range(k):
            m = 0
            for x in range(n):
                if rng.random() < 0.5:
                    m |= 1 << x
            vocab.append((f"p{i}", m))
        cs = classes(n, vocab)
        multi = [c for c in cs if c.bit_count() >= 2]
        if not multi:
            continue
        pos = neg = 0
        # Plant one to three collisions. One is the minimum for the task to exist; more than one
        # makes `splits` a count rather than a bit, which is what the metric needs to be graded on.
        planted = rng.sample(multi, min(len(multi), rng.randint(1, 3)))
        for c in planted:
            members = [x for x in range(n) if c >> x & 1]
            a, b = rng.sample(members, 2)
            pos |= 1 << a
            neg |= 1 << b
        for c in cs:
            if c in planted:
                continue
            inner = [x for x in range(n) if c >> x & 1]
            r = rng.random()
            if r < 0.35:
                pos |= 1 << rng.choice(inner)
            elif r < 0.70:
                neg |= 1 << rng.choice(inner)
        full = (1 << n) - 1
        unmeasured = full & ~(pos | neg)
        if unmeasured.bit_count() < 1 or (pos | neg).bit_count() < 4:
            continue
        return {
            "objects": n,
            "vocabulary": [{"name": nm, "ids": ids(m, n)} for nm, m in vocab],
            "pos": ids(pos, n),
            "neg": ids(neg, n),
            "unmeasured": ids(unmeasured, n),
            "colliding": len([1 for c in cs if c & pos and c & neg]),
        }
    raise SystemExit("could not generate a usable scenario; widen the parameters")


def to_request(sc: dict, request_id: str) -> dict:
    return {
        "protocol": PROTOCOL,
        "request_id": request_id,
        "objects": sc["objects"],
        "vocabulary": sc["vocabulary"],
        "evidence": {"pos": sc["pos"], "neg": sc["neg"]},
        "object_ids": list(range(sc["objects"])),
        "limit": 4,
        "ask": "invent one predicate that separates every colliding positive/negative pair",
    }


def witness_pair(sc: dict) -> tuple[int, int] | None:
    """One pair T2 says the vocabulary cannot separate: same signature, opposite labels.

    This is the definition `runtime::colliding_pairs` computes, recomputed here because
    `metaform propose` reports only a count (`splits`) and never the pair itself. That the protocol
    has no field for the witness is a gap in the product, not a convenience here: the whole claim is
    that the engine can name a pair, and the round-2 prompt had no way to quote one.
    """
    n = sc["objects"]
    vocab = [(a["name"], mask_of(a["ids"])) for a in sc["vocabulary"]]
    pos, neg = mask_of(sc["pos"]), mask_of(sc["neg"])
    for c in classes(n, vocab):
        p = [x for x in range(n) if (c >> x) & 1 and (pos >> x) & 1]
        q = [x for x in range(n) if (c >> x) & 1 and (neg >> x) & 1]
        if p and q:
            return (p[0], q[0])
    return None


def render(sc: dict, request: dict, round_no: int, first: dict | None,
           feedback: str = "diagnosis") -> str:
    """The prompt. Short on purpose: a 2B model has to be able to follow it.

    `feedback` decides what round 2 is told, and it exists so the measurement has a control:

      diagnosis  the engine's verdict, with the reason (this is the treatment)
      placebo    "rejected, try a different predicate" and no reason at all
      pairs      the verdict plus the specific pair that has to be separated
      none       the round-1 prompt again, byte for byte

    Without `placebo`, a recovery rate cannot be attributed to the engine: a second sample from the
    same model already changes the answer, and at a fixed q0 the expected recovery from resampling
    alone is not zero. The contrast that means something is diagnosis minus placebo, not diagnosis
    minus zero.
    """
    vocab = "; ".join(f"{a['name']} = {{{', '.join(map(str, a['ids']))}}}"
                      for a in request["vocabulary"])
    lines = [
        f"Objects are the integers 0..{sc['objects'] - 1}.",
        f"Known predicates: {vocab}",
        f"Positives: {sorted(sc['pos'])}",
        f"Negatives: {sorted(sc['neg'])}",
        f"Not measured (their labels are hidden): {sorted(sc['unmeasured'])}",
        "",
        "Some positive and some negative share the same values on every known predicate, so the",
        "known predicates cannot tell them apart. Invent ONE new predicate that separates them.",
        "It must not be a combination of the known predicates: a union, intersection or complement",
        "of them is constant on every group that shares a signature, and therefore separates nothing.",
        "",
        "Answer with JSON only, no prose:",
        '{"name": "<a short name>", "ids": [<object ids the predicate is true of>],',
        ' "warrant": "<why this set>", "falsifier": "<an observation that would refute it>",',
        f' "predict": {{"id": {min(sc["unmeasured"])}, "label": "pos" or "neg"}}}}',
        "",
        "`predict` is a bet on one unmeasured object; it is required.",
    ]
    if round_no == 2 and first is not None and feedback != "none":
        lines += ["", "Your previous answer was REJECTED by the checker."]
        if feedback == "diagnosis":
            lines += [
                f"  you proposed: ids = {first.get('ids')}",
                f"  the checker said: splits = {first.get('splits')} (it separated {first.get('splits')} of the pairs)",
                "  reason: a set built only from the known predicates cannot separate any pair, because it",
                "          takes the same value on both objects of every pair. A splits of 0 means exactly that.",
                "Propose a DIFFERENT predicate, using a distinction the known predicates do not encode.",
            ]
        elif feedback == "pairs":
            w = witness_pair(sc)
            if w is None:
                lines += ["Propose a DIFFERENT predicate."]
            else:
                a, b = w
                lines += [
                    f"  it separated 0 pairs.",
                    f"  objects {a} and {b} take the same value on every known predicate, and they have",
                    f"  different labels: object {a} is positive, object {b} is negative. Your set must",
                    f"  contain exactly one of them.",
                    "Propose a predicate that puts one of those two objects in and the other out.",
                ]
        else:
            lines += ["Propose a DIFFERENT predicate."]
    return "\n".join(lines)


def call_endpoint(url: str, model: str, prompt: str, timeout: int, grammar: str | None,
                  max_tokens: int = 512) -> str:
    """One completion, with the thinking channel turned off where the template allows it.

    A thinking model writes into `reasoning_content` and leaves `content` empty once the budget
    runs out. Scored as-is that is a protocol violation, so the metric would report the model
    cannot write JSON when what happened is that it thought for 300 tokens. `enable_thinking` is
    the Qwen3.x switch; templates that ignore it are unaffected. The fallback to
    `reasoning_content` covers servers that emit it anyway.
    """
    body = {
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.7,
        "max_tokens": max_tokens,
        "chat_template_kwargs": {"enable_thinking": False},
    }
    if grammar:
        body["grammar"] = grammar
    req = urllib.request.Request(
        url.rstrip("/") + "/v1/chat/completions",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as r:
        out = json.load(r)
    msg = out["choices"][0]["message"]
    return (msg.get("content") or "").strip() or (msg.get("reasoning_content") or "").strip()


def mock_answer(sc: dict, rng: random.Random, recombination_bias: float) -> dict:
    """A proposer that is weak in the specific way real ones are weak.

    With probability `recombination_bias` it answers with a set built out of the vocabulary, which is
    guaranteed to separate nothing. Otherwise it answers with a set that does separate. This is not a
    model: it is the plumbing's alibi, and any number it produces must be reported as synthetic.
    """
    n = sc["objects"]
    vocab = [(a["name"], mask_of(a["ids"])) for a in sc["vocabulary"]]
    cs = classes(n, vocab)
    pos, neg = mask_of(sc["pos"]), mask_of(sc["neg"])
    if rng.random() < recombination_bias:
        keep = [c for c in cs if rng.random() < 0.5]
        m = 0
        for c in keep:
            m |= c
        name, why = "union_of_known", "a union of known predicates"
    else:
        for _ in range(500):
            m = rng.getrandbits(n)
            if m and m != (1 << n) - 1 and splits(cs, pos, neg, m) > 0:
                break
        else:
            m = 1 << min(sc["pos"])
        name, why = "invented", "a set that is not constant on the shared signatures"
    bet = min(sc["unmeasured"])
    return {
        "name": name,
        "ids": ids(m, n),
        "warrant": why,
        "falsifier": "an object that satisfies the known predicates but has the other label",
        "predict": {"id": bet, "label": "pos" if bet in sc["pos"] else rng.choice(["pos", "neg"])},
    }


def judge(engine: Path, request: dict, candidate: dict | None, request_path: Path,
          response_path: Path, workdir: Path) -> dict:
    """Ask the real binary what the answer was worth. No judging happens in this file."""
    request_path.write_text(json.dumps(request), encoding="utf-8")
    if candidate is None:
        response_path.write_text(json.dumps({"protocol": PROTOCOL, "request_id": request["request_id"],
                                             "candidates": []}), encoding="utf-8")
    else:
        response_path.write_text(json.dumps({"protocol": PROTOCOL, "request_id": request["request_id"],
                                             "candidates": [candidate]}), encoding="utf-8")
    proc = subprocess.run([str(engine), "propose", "--request", str(request_path),
                           "--response", str(response_path)],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        return {"compliant": False, "splits": 0, "reason": f"engine exit {proc.returncode}"}
    try:
        out = json.loads(proc.stdout.strip().splitlines()[-1])
    except (IndexError, json.JSONDecodeError):
        return {"compliant": False, "splits": 0, "reason": "engine printed no JSON"}
    accepted = out.get("accepted") or []
    if accepted:
        best = max(int(c.get("splits", 0)) for c in accepted)
        return {"compliant": True, "splits": best, "reason": "accepted"}
    rejected = out.get("rejected") or []
    reason = rejected[0].get("reason") if rejected else "nothing accepted"
    return {"compliant": False, "splits": 0, "reason": str(reason)}


def parse_candidate(raw: str) -> dict | None:
    """The model's answer, salvaged from whatever prose it wrapped the JSON in.

    Every balanced `{...}` span is tried in order, rather than the first-brace-to-last-brace
    slice: a model that restates the expected shape in prose before answering would otherwise be
    parsed as one span running from its example to its answer, which is not JSON.
    """
    for i, ch in enumerate(raw):
        if ch != "{":
            continue
        depth = 0
        for j in range(i, len(raw)):
            if raw[j] == "{":
                depth += 1
            elif raw[j] == "}":
                depth -= 1
                if depth == 0:
                    try:
                        obj = json.loads(raw[i:j + 1])
                    except json.JSONDecodeError:
                        break
                    if isinstance(obj, dict) and isinstance(obj.get("ids"), list):
                        return obj
                    break
    return None


def cmd_instances(args) -> int:
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)
    scenes = []
    for i in range(args.n):
        sc = scene(rng, args.objects, args.atoms)
        sc["id"] = f"q{i:02d}"
        sc["request"] = to_request(sc, f"local-{sc['id']}")
        scenes.append(sc)
    path = out / "instances.json"
    path.write_text(json.dumps(scenes, indent=1), encoding="utf-8")
    coll = sum(s["colliding"] for s in scenes)
    print(f"instances: {len(scenes)} written to {path}")
    print(f"colliding classes over the set: {coll} (min {min(s['colliding'] for s in scenes)}, "
          f"max {max(s['colliding'] for s in scenes)})")
    return 0


def cmd_prompt(args) -> int:
    scenes = json.loads(Path(args.instances).read_text(encoding="utf-8"))
    first = {}
    if args.first:
        first = {a["id"]: a for a in json.loads(Path(args.first).read_text(encoding="utf-8"))}
    prompts = []
    for sc in scenes:
        prompts.append({
            "id": sc["id"],
            "round": args.round,
            "prompt": render(sc, sc["request"], args.round, first.get(sc["id"]), args.feedback),
        })
    Path(args.out).write_text(json.dumps(prompts, indent=1), encoding="utf-8")
    print(f"prompts: {len(prompts)} written to {args.out} (round {args.round})")
    if args.show:
        print("-" * 72)
        print(prompts[0]["prompt"])
        print("-" * 72)
    return 0


def cmd_answer(args) -> int:
    prompts = json.loads(Path(args.prompts).read_text(encoding="utf-8"))
    scenes = {s["id"]: s for s in json.loads(Path(args.instances).read_text(encoding="utf-8"))}
    rng = random.Random(args.seed)
    answers = []
    for p in prompts:
        sc = scenes[p["id"]]
        if args.mock:
            cand = mock_answer(sc, rng, args.recombination_bias)
            raw = json.dumps(cand)
        else:
            try:
                raw = call_endpoint(args.endpoint, args.model, p["prompt"], args.timeout,
                                    args.grammar, args.max_tokens)
            except OSError as e:
                raise SystemExit(
                    f"cannot reach {args.endpoint}: {e}\n"
                    "start the server first, e.g.\n"
                    "  llama-server -m Phi-4-mini-instruct-Q4_K_M.gguf -ngl 99 -c 4096 --port 8080 --jinja"
                ) from e
        cand = parse_candidate(raw)
        answers.append({
            "id": p["id"],
            "round": p["round"],
            "raw": raw[:2000],
            "candidate": cand,
            "parsed": cand is not None,
        })
        mark = "parsed" if cand else "UNPARSED"
        print(f"  {p['id']} round {p['round']}: {mark}")
    Path(args.out).write_text(json.dumps(answers, indent=1), encoding="utf-8")
    ok = sum(1 for a in answers if a["parsed"])
    print(f"answers: {len(answers)} written to {args.out}; json parsed {ok}/{len(answers)}")
    return 0


def cmd_score(args) -> int:
    engine = engine_path(args.engine)
    scenes = json.loads(Path(args.instances).read_text(encoding="utf-8"))
    first = {a["id"]: a for a in json.loads(Path(args.first).read_text(encoding="utf-8"))}
    second = {}
    if args.second:
        second = {a["id"]: a for a in json.loads(Path(args.second).read_text(encoding="utf-8"))}
    work = Path(args.workdir)
    work.mkdir(parents=True, exist_ok=True)

    rows = []
    for sc in scenes:
        a1 = first.get(sc["id"])
        j1 = judge(engine, sc["request"], a1["candidate"] if a1 else None,
                   work / f"{sc['id']}_req.json", work / f"{sc['id']}_r1.json", work)
        row = {"id": sc["id"], "r1_parsed": bool(a1 and a1["parsed"]),
               "r1_splits": j1["splits"], "r1_zero": j1["splits"] == 0, "r1_reason": j1["reason"]}
        a2 = second.get(sc["id"])
        if a2 is not None:
            j2 = judge(engine, sc["request"], a2["candidate"],
                       work / f"{sc['id']}_req.json", work / f"{sc['id']}_r2.json", work)
            row.update({"r2_parsed": bool(a2["parsed"]), "r2_splits": j2["splits"],
                        "r2_zero": j2["splits"] == 0, "r2_reason": j2["reason"]})
        rows.append(row)

    n = len(rows)
    # q0 is a share of the answers the engine could judge at all. An answer that would not parse is
    # a protocol failure, not a recombination, and folding it into this denominator reports "the
    # model cannot write JSON" as "the model recombines", which are different findings with
    # different fixes.
    parsed_rows = [r for r in rows if r["r1_parsed"]]
    zero1 = [r for r in parsed_rows if r["r1_zero"]]
    q0 = len(zero1) / len(parsed_rows) if parsed_rows else 0.0

    print(f"instances            {n}")
    print(f"json parsed (round 1) {len(parsed_rows)}/{n}  ({len(parsed_rows) / n:.0%})")
    print(f"q0  splits==0         {len(zero1)}/{len(parsed_rows)}  ({q0:.0%}) of parsed")
    print(f"mean splits round 1   {sum(r['r1_splits'] for r in parsed_rows) / len(parsed_rows):.2f}"
          if parsed_rows else "mean splits round 1   n/a")
    if second:
        recovered = [r for r in zero1 if r.get("r2_parsed") and r.get("r2_splits", 0) > 0]
        q1 = (len(zero1) - len(recovered)) / len(zero1) if zero1 else 0.0
        print(f"q1  still 0 after feedback  {len(zero1) - len(recovered)}/{len(zero1)}  ({q1:.0%})")
        print(f"engine contribution   q0 - q1 = {q0 - q1:+.2f}  "
              f"({len(recovered)} of {len(zero1)} bad answers fixed by one message)")
        r2 = [r for r in rows if r.get("r2_parsed")]
        if r2:
            print(f"mean splits round 2   {sum(r['r2_splits'] for r in r2) / len(r2):.2f}")
    tail = Path(args.tsv)
    tail.write_text("id\tr1_splits\tr1_zero\tr1_reason\tr2_splits\tr2_zero\tr2_reason\n"
                    + "".join(f"{r['id']}\t{r['r1_splits']}\t{int(r['r1_zero'])}\t{r['r1_reason']}\t"
                              f"{r.get('r2_splits', '')}\t{int(r.get('r2_zero', 0))}\t{r.get('r2_reason', '')}\n"
                              for r in rows), encoding="utf-8")
    print(f"per-instance rows     {tail}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("instances", help="generate reproducible scenarios")
    p.add_argument("--n", type=int, default=30)
    p.add_argument("--objects", type=int, default=10)
    p.add_argument("--atoms", type=int, default=3)
    p.add_argument("--seed", type=int, default=7)
    p.add_argument("--out", required=True)
    p.set_defaults(fn=cmd_instances)

    p = sub.add_parser("prompt", help="render the prompts for one round")
    p.add_argument("--instances", required=True)
    p.add_argument("--round", type=int, default=1, choices=[1, 2])
    p.add_argument("--first", help="round-1 answers, so round 2 can quote the rejection")
    p.add_argument("--feedback", default="diagnosis", choices=["diagnosis", "pairs", "placebo", "none"],
                   help="what round 2 is told: the engine's reason, the witness pair, a bare rejection, or nothing")
    p.add_argument("--show", action="store_true")
    p.add_argument("--out", required=True)
    p.set_defaults(fn=cmd_prompt)

    p = sub.add_parser("answer", help="fill prompts from a model, or from the mock")
    p.add_argument("--prompts", required=True)
    p.add_argument("--instances", required=True)
    p.add_argument("--mock", action="store_true")
    p.add_argument("--recombination-bias", type=float, default=0.8)
    p.add_argument("--endpoint", default="http://127.0.0.1:8080")
    p.add_argument("--model", default="local")
    p.add_argument("--timeout", type=int, default=180)
    p.add_argument("--grammar", default=None,
                   help="a GBNF grammar file passed to llama.cpp, to remove JSON failures from the metric")
    p.add_argument("--max-tokens", type=int, default=768,
                   help="completion budget; a reasoning model needs room before its JSON")
    p.add_argument("--seed", type=int, default=11)
    p.add_argument("--out", required=True)
    p.set_defaults(fn=cmd_answer)

    p = sub.add_parser("score", help="ask the engine what the answers were worth")
    p.add_argument("--instances", required=True)
    p.add_argument("--first", required=True)
    p.add_argument("--second")
    p.add_argument("--engine", default=None)
    p.add_argument("--workdir", default="tmp/q/work")
    p.add_argument("--tsv", default="tmp/q/rows.tsv")
    p.set_defaults(fn=cmd_score)

    args = ap.parse_args()
    if getattr(args, "grammar", None):
        args.grammar = Path(args.grammar).read_text(encoding="utf-8")
    return args.fn(args)


if __name__ == "__main__":
    sys.exit(main())
