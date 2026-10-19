#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: run-variable-lens.sh <manifest.json> <forensic-report.json> <local-coverage.json> <private-output-dir>" >&2
  exit 2
fi

manifest=$1
forensic=$2
coverage=$3
output_dir=$4
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
python_bin=${TOUPERA_SLITHER_PYTHON:-python3}
engine_bin=${TOUPERA_SECURITY_ENGINE:-security-engine}

source_path=$(jq -er '.analysis_source_path' "$manifest")
solc_path=$(jq -er '.solc_path' "$manifest")
evm_version=$(jq -er '.evm_version // "cancun"' "$manifest")

umask 077
mkdir -p -- "$output_dir"
chmod 700 -- "$output_dir"
"$python_bin" "$script_dir/export-variable-ssa.py" "$source_path" "$solc_path" \
  --evm-version "$evm_version" > "$output_dir/slither-ssa.json"
"$engine_bin" --variable-lens "$manifest" "$forensic" "$coverage" \
  "$output_dir/slither-ssa.json" > "$output_dir/report.json"
