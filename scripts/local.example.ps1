# Copy this file to `local.ps1` (same directory) and fill in the paths for your machine.
# `local.ps1` is gitignored: it holds machine-local facts, not repository content.

$env:METAFORM_PYTHON = '<full path to python.exe>'

# A Lake project whose environment can load Mathlib (see check-lean.ps1).
$env:METAFORM_LEAN_PROJECT = '<full path to a Lake project with Mathlib>'

# Optional: where the Mathlib build cache lives.
# $env:MATHLIB_CACHE_DIR = '<full path to the Mathlib build cache>'
