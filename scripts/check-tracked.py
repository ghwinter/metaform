"""Reject tracked prose that is not a formal document.

The repository keeps code, scripts, configuration, and documents that state what the system is and
what it has measured. Research notes on other libraries, local facts about this machine, and
conversation transcripts belong in `research/` or `tmp/`, which are not tracked -- a rule that is
only prose will rot, so it is a gate.

Allowed tracked markdown:
  README.md, README.zh.md, CHARTER.md, CHARTER.zh.md   the two documents that define the project
  .dsh/skills/**                                     the skill the host reads
  experiments/RESULTS.md, experiments/RESULTS.zh.md  the evidence ledger
  proofs/**                                          formal statements about the mechanisation
  docs/**                                            reserved for formal documents only

Anything else fails, with the instruction to move it under research/ or tmp/.
"""
import subprocess
import sys

ALLOWED_PREFIXES = (".dsh/skills/", "proofs/", "docs/")
ALLOWED_EXACT = {
    "README.md",
    "README.zh.md",
    "CHARTER.md",
    "CHARTER.zh.md",
    "experiments/RESULTS.md",
    "experiments/RESULTS.zh.md",
}


def tracked_markdown():
    out = subprocess.run(["git", "ls-files", "*.md"], capture_output=True, text=True, check=True)
    return [line.strip().replace("\\", "/") for line in out.stdout.splitlines() if line.strip()]


def main():
    bad = []
    for path in tracked_markdown():
        if path in ALLOWED_EXACT or path.startswith(ALLOWED_PREFIXES):
            continue
        bad.append(path)
    if bad:
        for path in bad:
            print(f"tracked prose is not a formal document: {path}")
        print("move it under research/ or tmp/ (both untracked); see scripts/check-tracked.py")
        return 1
    print(f"[check-tracked] ok: {len(tracked_markdown())} tracked document(s), all formal")
    return 0


if __name__ == "__main__":
    sys.exit(main())
