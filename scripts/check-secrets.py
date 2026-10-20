#!/usr/bin/env python3
"""Scan tracked files and explicit working-tree paths for API-key-shaped strings.

Patterns:

  api-key-literal      the sk- prefix followed by 16 or more token characters
  api-key-assignment   DEEPSEEK_API_KEY or ARK_CODE_LATEST_API_KEY assigned a
                       non-empty value that is not an environment reference
  quoted-key-blob      40+ base64-ish characters inside quotes on a line that
                       also mentions "key"

Default paths: everything in `git ls-files`, plus tmp/ when it exists. Explicit
path arguments are scanned in addition to the tracked set. A hit prints
`path:line: pattern-name` and at most the first six characters of the match;
the scanner never prints more of a secret than that. A clean run prints the
number of files scanned.

`--self-test` classifies embedded positive and negative samples. The samples
are assembled at run time, so this source file does not contain a key literal.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_BYTES = 2 * 1024 * 1024
KEY_NAMES = ("DEEPSEEK_API_KEY", "ARK_CODE_LATEST_API_KEY")

SK_TOKEN = re.compile(r"(?<![A-Za-z0-9_-])sk-[A-Za-z0-9_-]{16,}(?![A-Za-z0-9_-])")
ASSIGN = re.compile(
    r"(?<![A-Za-z0-9_])(?P<name>" + "|".join(KEY_NAMES) + r")\s*=\s*(?P<value>.+)$"
)
BLOB = re.compile(r"(?P<q>['\"])(?P<blob>[A-Za-z0-9+/=_-]{40,})(?P=q)")


@dataclass(frozen=True)
class Hit:
    """One match: its line, pattern name, and at most a six-character prefix."""

    line: int
    pattern: str
    prefix: str


def _unquote(raw: str) -> str:
    """Return the value part of an assignment, dropping quotes and comments."""
    value = raw.strip()
    if not value:
        return ""
    if value[0] in "\"'":
        end = value.find(value[0], 1)
        if end != -1:
            return value[1:end].strip()
        return value[1:].strip()
    value = re.split(r"\s+#", value, maxsplit=1)[0].strip()
    return value.rstrip(";").strip()


def _is_env_reference(value: str) -> bool:
    """True when the assigned text names an environment source, not a literal."""
    v = value.strip()
    if not v:
        return True
    low = v.lower()
    if v[0] in "$%!":
        return True
    prefixes = (
        "os.environ",
        "os.getenv",
        "process.env",
        "system.getenv",
        "environment.",
        "secrets.",
        "env[",
        "env(",
        "getenv(",
        "env:",
    )
    if low.startswith(prefixes):
        return True
    if "{{" in v and "}}" in v:
        return True
    if v.startswith("<") and v.endswith(">"):
        return True
    if low in {"null", "none", "true", "false"}:
        return True
    # A bare variable name is a reference, not a literal value.
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", v) and v.isupper():
        return True
    return False


def _looks_placeholder(value: str) -> bool:
    """Documentation placeholders, so the scanner does not flag its own docs."""
    low = value.lower()
    if re.fullmatch(r"\.{3,}|…|\*{3,}|x{3,}|-{3,}", low):
        return True
    markers = (
        "your_key",
        "your-key",
        "your key",
        "changeme",
        "change-me",
        "replace_me",
        "replace-me",
        "placeholder",
        "example",
        "dummy",
        "todo",
    )
    return any(marker in low for marker in markers)


def _blobish(blob: str) -> bool:
    """Exclude a run of separators; a base64-ish blob needs an alphanumeric."""
    return bool(re.search(r"[A-Za-z0-9]", blob))


def scan_text(text: str) -> list[Hit]:
    """Return every hit in one text blob, with one-based line numbers."""
    hits: list[Hit] = []
    for lineno, line in enumerate(text.splitlines(), 1):
        for match in SK_TOKEN.finditer(line):
            hits.append(Hit(lineno, "api-key-literal", match.group(0)[:6]))
        match = ASSIGN.search(line)
        if match:
            value = _unquote(match.group("value"))
            if value and not _is_env_reference(value) and not _looks_placeholder(value):
                hits.append(Hit(lineno, "api-key-assignment", value[:6]))
        if "key" in line.lower():
            for match in BLOB.finditer(line):
                blob = match.group("blob")
                if _blobish(blob):
                    hits.append(Hit(lineno, "quoted-key-blob", blob[:6]))
    return hits


def scan_file(path: Path) -> list[Hit] | None:
    """Scan one file; None means binary, oversized, or unreadable."""
    try:
        data = path.read_bytes()
    except OSError:
        return None
    if len(data) > MAX_BYTES or b"\x00" in data[:8192]:
        return None
    return scan_text(data.decode("utf-8", errors="replace"))


def tracked_paths() -> list[Path]:
    """Every tracked path, whether or not it still exists in the working tree."""
    result = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z"],
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError("git ls-files failed")
    names = result.stdout.decode("utf-8", errors="replace").split("\0")
    return [ROOT / name for name in names if name]


def walk_path(path: Path):
    """Yield regular files under an explicit path, skipping VCS internals."""
    if path.is_file():
        yield path
        return
    if not path.is_dir():
        return
    for root, dirs, files in os.walk(path):
        dirs[:] = sorted(d for d in dirs if d != ".git" and d != "__pycache__")
        for name in sorted(files):
            yield Path(root) / name


def display_path(path: Path) -> str:
    """Prefer a repository-relative POSIX path; fall back to the absolute path."""
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except (OSError, ValueError):
        return str(path)


def _s(*parts: str) -> str:
    """Assemble a sample so no complete literal appears in this source file."""
    return "".join(parts)


def self_test() -> int:
    """Classify embedded samples; exit 0 only when every one is correct."""
    positives = [
        ("api-key-literal", _s("sk-", "A1b2C3d4E5f6G7h8I9j0")),
        ("api-key-assignment", _s("DEEPSEEK_API_KEY", "=", "a1b2c3d4e5f6")),
        (
            "api-key-assignment",
            _s("ARK_CODE_LATEST_API_KEY", " = ", '"', "Zx9Y8w7V6u5T4s3R2q1P", '"'),
        ),
        (
            "quoted-key-blob",
            _s(
                "api_key = ",
                '"',
                "A1b2C3d4E5f6G7h8I9j0",
                "K1l2M3n4O5p6Q7r8S9t0",
                '"',
            ),
        ),
    ]
    negatives = [
        _s("DEEPSEEK_API_KEY", " is read from the environment"),
        _s("DEEPSEEK_API_KEY", "=", "$env:", "DEEPSEEK_API_KEY"),
        _s("ARK_CODE_LATEST_API_KEY", " = ", "${", "ARK_CODE_LATEST_API_KEY", "}"),
        _s("DEEPSEEK_API_KEY", " = ", "os.environ[", "'DEEPSEEK_API_KEY'", "]"),
        _s("sk-", "short"),
        _s(
            "key = ",
            '"',
            "A1b2C3d4E5f6G7h8I9j0",
            "K1l2M3n4O5p6Q7r8S9",
            '"',
        ),
        _s(
            "blob = ",
            '"',
            "A1b2C3d4E5f6G7h8I9j0",
            "K1l2M3n4O5p6Q7r8S9t0",
            '"',
        ),
        _s("DEEPSEEK_API_KEY", " = ", '""'),
        _s("DEEPSEEK_API_KEY", " = ", "<your-key>"),
        _s("DEEPSEEK_API_KEY", " = ", "DEEPSEEK_API_KEY"),
    ]

    ok = True
    for expected, sample in positives:
        hits = scan_text(sample)
        good = any(hit.pattern == expected for hit in hits)
        print(f"  {'ok  ' if good else 'FAIL'} positive {expected}")
        ok = ok and good
    for index, sample in enumerate(negatives, 1):
        hits = scan_text(sample)
        good = not hits
        print(f"  {'ok  ' if good else 'FAIL'} negative {index}")
        if not good:
            names = ", ".join(sorted({hit.pattern for hit in hits}))
            print(f"       unexpected: {names}")
        ok = ok and good
    state = "ok" if ok else "FAIL"
    print(
        f"[check-secrets] self-test {state}: "
        f"{len(positives)} positive, {len(negatives)} negative"
    )
    return 0 if ok else 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Find API-key-shaped strings.")
    parser.add_argument("paths", nargs="*", help="extra files or directories to scan")
    parser.add_argument("--self-test", action="store_true", help="classify embedded samples")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    try:
        candidates = tracked_paths()
    except (OSError, RuntimeError) as error:
        print(f"[check-secrets] cannot list tracked files: {error}")
        return 2

    if args.paths:
        for raw in args.paths:
            path = Path(raw)
            if not path.is_absolute():
                path = Path.cwd() / path
            if not path.exists():
                print(f"[check-secrets] path not found: {raw}")
                return 2
            candidates.append(path)
    else:
        tmp = ROOT / "tmp"
        if tmp.is_dir():
            candidates.append(tmp)

    paths: list[Path] = []
    seen: set[Path] = set()
    for candidate in candidates:
        for path in walk_path(candidate):
            try:
                resolved = path.resolve()
            except OSError:
                continue
            if resolved in seen:
                continue
            seen.add(resolved)
            paths.append(path)
    paths.sort(key=display_path)

    hits: list[tuple[Path, Hit]] = []
    scanned = 0
    for path in paths:
        found = scan_file(path)
        if found is None:
            continue
        scanned += 1
        hits.extend((path, hit) for hit in found)

    if hits:
        for path, hit in hits:
            print(f"{display_path(path)}:{hit.line}: {hit.pattern} (prefix: {hit.prefix})")
        print(f"[check-secrets] {len(hits)} hit(s)")
        return 1

    print(f"[check-secrets] ok: scanned {scanned} file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
