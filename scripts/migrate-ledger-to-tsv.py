"""
One-off migration: convert the Python ledger probe_ledger.json into the Rust append-only
TSV (.metaform/ledger.tsv).

TSV columns: id  case  status  by  round  reliability  answer  reason
If the same id appears more than once (historical edits), write every occurrence in JSON
order -- the Rust side keeps the last one when folding.

Run: python scripts/migrate-ledger-to-tsv.py
"""
from __future__ import annotations

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "experiments" / "probe_ledger.json"
DST = ROOT / ".metaform" / "ledger.tsv"

HEADER = "# metaform ledger v1\nid\tcase\tstatus\tby\tround\treliability\tanswer\treason\n"


def clean(s: str) -> str:
    return s.replace("\t", " ").replace("\n", " ").replace("\r", " ")


def main() -> None:
    data = json.loads(SRC.read_text(encoding="utf-8"))
    entries = data.get("entries", [])
    DST.parent.mkdir(parents=True, exist_ok=True)
    lines = [HEADER.rstrip("\n")]
    for e in entries:
        ev = e.get("evidence", {})
        lines.append(
            "\t".join(
                [
                    clean(e["id"]),
                    str(e["case"]),
                    clean(e.get("status", "active")),
                    clean(e.get("by", "migrated")),
                    str(e.get("round", 0)),
                    f"{float(ev.get('reliability', 1.0)):.2f}",
                    "true" if ev.get("answer", True) else "false",
                    clean(e.get("reason", "")),
                ]
            )
        )
    DST.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"migrated {len(entries)} entries → {DST}")


if __name__ == "__main__":
    main()
