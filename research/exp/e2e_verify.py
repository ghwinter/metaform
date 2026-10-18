"""
端到端检查，两段：

1. 脚本模式：`scenarios/verify.demo` 走
   VERIFY → PROPOSE → ASK → STOP，并核对每一步的 JSON。
2. 项目模式：用 `init / state / ledger / report` 在临时目录里建一个真实项目，
   检查报告在探测前后确实换了判定（VERIFY → INSUFFICIENT + 撞车 (0,4)）。

这个域不是论文里的 A/B/C：6 个测试文件，判“是不是集成测试”。
它要证明的是：**先有 VERIFY，才谈 PROPOSE**——初始词汇下所有等价类都有标签，
但 4 与 5 的标签是从同类继承的。

运行：python e2e_verify.py [--engine ...] [--no-build]
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEMO = ROOT / "engine" / "scenarios" / "verify.demo"
DEFAULT_ENGINE = ROOT / "engine" / "target" / "debug" / "metaform.exe"
MANIFEST = ROOT / "engine" / "Cargo.toml"


def build() -> None:
    r = subprocess.run(
        ["cargo", "build", "--quiet", "--manifest-path", str(MANIFEST)],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit(f"cargo build 失败:\n{r.stderr[:600]}")


def run(engine: Path) -> list[dict]:
    out = subprocess.run(
        [str(engine), str(DEMO)], capture_output=True, text=True, encoding="utf-8"
    )
    if out.returncode != 0:
        raise SystemExit(f"engine 退出码 {out.returncode}: {out.stderr[:400]}")
    rows = []
    for line in out.stdout.splitlines():
        line = line.strip()
        if line.startswith("{"):
            rows.append(json.loads(line))
    return rows


def cli(engine: Path, *args: str) -> str:
    r = subprocess.run([str(engine), *args], capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(f"`{' '.join(args)}` 退出码 {r.returncode}: {r.stderr[:300]}")
    return r.stdout


def last_json(out: str) -> dict:
    for line in reversed(out.splitlines()):
        if line.strip().startswith("{"):
            return json.loads(line)
    raise SystemExit("输出里没有 JSON")


def project_mode(engine: Path) -> None:
    d = Path(tempfile.mkdtemp(prefix="mf-e2e-"))
    try:
        cli(engine, "init", "--project", str(d))
        cli(engine, "state", "init", "--project", str(d), "--n", "6")
        cli(engine, "state", "add", "--project", str(d), "--name", "slow", "--ids", "0,1,4")
        cli(engine, "state", "add", "--project", str(d), "--name", "doc", "--ids", "0,2,4,5")
        for i, (yes, no) in enumerate([(3, 0), (0, 3), (3, 0), (0, 3)]):
            cli(
                engine, "state", "vote", "--project", str(d),
                "--id", str(i), "--yes", str(yes), "--no", str(no),
            )

        need = last_json(cli(engine, "state", "need", "--project", str(d)))
        assert need["action"] == "VERIFY", need
        assert need["unverified"] == [4, 5], need

        cli(engine, "report", "--project", str(d))
        before = (d / ".metaform" / "out" / "report.md").read_text(encoding="utf-8")
        assert "VERIFY" in before, before
        assert "## 待验证" in before, before
        assert "4, 5" in before, before

        cli(
            engine, "state", "vote", "--project", str(d),
            "--id", "4", "--yes", "0", "--no", "3",
            "--record", "--by", "probe", "--reason", "探测 4：与 0 撞车",
        )
        cli(engine, "report", "--project", str(d))
        after = (d / ".metaform" / "out" / "report.md").read_text(encoding="utf-8")
        assert "INSUFFICIENT" in after, after
        assert "(0,4)" in after, after
        assert "探测 4：与 0 撞车" in after, "台账证据要出现在报告里"

        print("project: report VERIFY -> INSUFFICIENT after probe")
    finally:
        shutil.rmtree(d, ignore_errors=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", default=str(DEFAULT_ENGINE))
    ap.add_argument("--no-build", action="store_true")
    args = ap.parse_args()

    engine = Path(args.engine)
    if not args.no_build:
        build()

    rows = run(engine)
    steps = [(r["status"], r["action"]) for r in rows]
    print("steps:", " -> ".join(a for _, a in steps))

    assert len(rows) == 4, f"期望 4 步，得到 {len(rows)}: {steps}"
    assert steps[0] == ("IDENTIFIED", "VERIFY"), steps[0]
    assert steps[1] == ("INSUFFICIENT", "PROPOSE"), steps[1]
    assert steps[2] == ("UNDETERMINED", "ASK"), steps[2]
    assert steps[3] == ("IDENTIFIED", "STOP"), steps[3]

    assert rows[0]["unverified"] == [4, 5], rows[0]
    assert rows[1]["colliding_pairs"] == [[0, 4]], rows[1]
    assert rows[1]["unverified"] == [5], "探测 4 之后，5 仍未验证"
    assert rows[3]["unverified"] == [], "最终停机时不应再有继承标签"

    project_mode(engine)

    print("OK：脚本模式 VERIFY→PROPOSE→ASK→STOP；项目模式的报告随探测改变")


if __name__ == "__main__":
    main()
