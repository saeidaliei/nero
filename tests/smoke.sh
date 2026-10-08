#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# This script is a simple shell-level smoke test for the expected CLI UX.
# It requires a compiled `nero` binary at $NERO_BIN.
NERO_BIN="${NERO_BIN:-$ROOT/target/debug/nero}"

if [[ ! -x "$NERO_BIN" ]]; then
  echo "nero binary not found: $NERO_BIN" >&2
  exit 2
fi

"$NERO_BIN" init "$TMP"
cd "$TMP"
"$NERO_BIN" new "Fourier Transform"
printf '# Fourier Transform\n\nSee [[Signal Processing]].\n' > fourier-transform.md
"$NERO_BIN" new "Signal Processing"
"$NERO_BIN" find fourier
"$NERO_BIN" backlinks "Signal Processing"
"$NERO_BIN" today
"$NERO_BIN" doctor
