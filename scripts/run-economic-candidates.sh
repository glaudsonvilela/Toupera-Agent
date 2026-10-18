#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo 'usage: run-economic-candidates.sh <single-source.sol> <new-output-directory>' >&2
  exit 2
fi

input=$(realpath -- "$1")
if [[ ! -f "$input" ]]; then
  echo "source file not found: $input" >&2
  exit 2
fi
output=$(realpath -m -- "$2")
if [[ -e "$output" ]]; then
  echo "output directory already exists: $output" >&2
  exit 2
fi
mkdir -p -- "$(dirname -- "$output")"
mkdir -- "$output"
source_copy="$output/$(basename -- "$input")"
cp -- "$input" "$source_copy"

engine=${TOUPERA_SECURITY_ENGINE:-security-engine}
solc=${TOUPERA_SOLC_BIN:-solc}
slither=${TOUPERA_SLITHER_BIN:-slither}
"$solc" --version > "$output/solc-version.txt"
"$slither" --version > "$output/slither-version.txt"

# Slither uses a nonzero exit code when it reports detector signals. The Rust
# importer requires success=true in the generated JSON before accepting it.
slither_status=0
timeout --signal=TERM --kill-after=10s 300s \
  "$slither" "$source_copy" --solc "$solc" --json "$output/slither.json" \
  --disable-color >&2 || slither_status=$?
if [[ ! -s "$output/slither.json" ]]; then
  echo "Slither did not produce JSON (exit $slither_status)" >&2
  exit 1
fi

"$engine" --economic-candidates "$source_copy" "$solc" "$output/slither.json" \
  | tee "$output/candidates.json"
