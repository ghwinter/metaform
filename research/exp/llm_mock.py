"""
Mock external model for the `metaform.propose/1` boundary.

Reads a protocol request and writes a deterministic response. The four cases exercise what the
engine must handle:

  good    one valid candidate, one out-of-range candidate, one duplicate-name candidate
  bad     the wrong protocol, so the whole response must be rejected
  refuse  a `refused` string: the model declines and nothing is accepted
  loop    the three candidates of `propose_loop.py`: `net` (usable), `slow` (duplicate name),
          `ghost` (out of range)

The mock never reads the engine and never touches state: it is the "external model" side of the
boundary.

Run: python research/exp/llm_mock.py --request req.json --response resp.json --case good
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

PROTOCOL = "metaform.propose/1"


def load_request(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def first_positive(req: dict) -> int:
    """The lowest positive object, or 0 when the request carries no positive evidence."""
    pos = sorted(set(req.get("evidence", {}).get("pos", [])))
    return pos[0] if pos else 0


def duplicate_name(req: dict) -> str:
    """Reuse a vocabulary name when possible, otherwise reuse the mock's own valid name."""
    vocab = req.get("vocabulary", [])
    if vocab and isinstance(vocab[0], dict) and isinstance(vocab[0].get("name"), str):
        return vocab[0]["name"]
    return "mock-valid"


def good_response(req: dict) -> dict:
    n = int(req.get("objects", 0))
    candidate_ids = [first_positive(req)]
    candidates = [
        {
            "name": "mock-valid",
            "ids": candidate_ids,
            "warrant": "the first positive object is a member and no negative object is included",
            "falsifier": "a negative object that this predicate would also include",
        },
        {
            "name": "mock-out-of-range",
            "ids": [n + 5],
            "warrant": "deliberately invalid: the id is beyond the object count",
            "falsifier": "every id must be smaller than `objects`",
        },
        {
            "name": duplicate_name(req),
            "ids": candidate_ids,
            "warrant": "deliberately invalid: the name is already taken",
            "falsifier": "candidate names must be fresh",
        },
    ]
    return {"protocol": PROTOCOL, "request_id": req.get("request_id"), "candidates": candidates}


def bad_response(req: dict) -> dict:
    # Same shape as `good`, but rule 1 must fire before any candidate is considered.
    response = good_response(req)
    response["protocol"] = "metaform.propose/0"
    return response


def refuse_response(req: dict) -> dict:
    return {
        "protocol": PROTOCOL,
        "request_id": req.get("request_id"),
        "candidates": [],
        "refused": "not enough evidence to propose a predicate",
    }


def loop_response(req: dict) -> dict:
    """The three candidates of one proposal round on the verify.demo state.

    `net` covers the request's positive evidence (objects 0 and 2); `slow` reuses the first
    vocabulary name, so the engine must reject it; `ghost` includes an id past `objects`.
    """
    n = int(req.get("objects", 0))
    candidates = [
        {
            "name": "net",
            "ids": [0, 2],
            "warrant": "objects 0 and 2 are the positives that separate the colliding class under slow and doc",
            "falsifier": "a negative object that `net` would also include",
        },
        {
            "name": duplicate_name(req),
            "ids": [0],
            "warrant": "deliberately invalid: the name is already taken",
            "falsifier": "candidate names must be fresh",
        },
        {
            "name": "ghost",
            "ids": [0, n + 3],
            "warrant": "deliberately invalid: the second id is beyond the object count",
            "falsifier": "every id must be smaller than `objects`",
        },
    ]
    return {"protocol": PROTOCOL, "request_id": req.get("request_id"), "candidates": candidates}


BUILDERS = {"good": good_response, "bad": bad_response, "refuse": refuse_response, "loop": loop_response}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--request", required=True, help="request JSON written by the caller")
    ap.add_argument("--response", required=True, help="response JSON to write")
    ap.add_argument("--case", choices=sorted(BUILDERS), default="good")
    args = ap.parse_args()

    request = load_request(Path(args.request))
    response = BUILDERS[args.case](request)
    # Deterministic content: fixed key order, fixed newline, no timestamps.
    Path(args.response).write_text(json.dumps(response, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
