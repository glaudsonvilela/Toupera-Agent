#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo 'usage: run-bytecode-companions.sh <runtime.hex> <new-output-directory>' >&2
  exit 2
fi

input=$(realpath -- "$1")
if [[ ! -f "$input" ]]; then
  echo "runtime file not found: $input" >&2
  exit 2
fi

engine=${TOUPERA_SECURITY_ENGINE:-security-engine}
heimdall=${TOUPERA_HEIMDALL_BIN:-heimdall}
image=${TOUPERA_GIGAHORSE_IMAGE:-ghcr.io/nevillegrech/gigahorse-toolchain@sha256:f676ca8aaf88acd47be27ed1967acddc9c99acdd041b34e79472cfb028910743}
cache=${TOUPERA_GIGAHORSE_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/toupera-gigahorse}
work=$(realpath -m -- "$2")
if [[ -e "$work" ]]; then
  echo "output directory already exists: $work" >&2
  exit 2
fi
mkdir -p -- "$(dirname -- "$work")"
mkdir -- "$work"
mkdir -p "$work/heimdall" "$work/gigahorse-temp" "$cache"
cp -- "$input" "$work/runtime.hex"
"$heimdall" --version > "$work/heimdall-version.txt"
podman image inspect "$image" --format '{{.Digest}}' > "$work/gigahorse-image-digest.txt"

# Heimdall reads local bytecode and does not resolve signatures through a
# remote service. Source/LLM postprocessing is deliberately not enabled.
timeout --signal=TERM --kill-after=10s 300s \
  "$heimdall" decompile "$work/runtime.hex" --skip-resolving --default \
  --output "$work/heimdall" --quiet --color never >&2

# The official Gigahorse image runs its local Datalog lifter. Only a temporary
# input/output directory and compilation cache are mounted; networking is off.
timeout --signal=TERM --kill-after=10s 1200s \
  podman run --rm --network none --userns keep-id \
  --mount "type=bind,source=$work/runtime.hex,target=/input/runtime.hex,readonly" \
  --mount "type=bind,source=$work/gigahorse-temp,target=/opt/gigahorse/gigahorse-toolchain/.temp" \
  --mount "type=bind,source=$cache,target=/opt/gigahorse/gigahorse-toolchain/cache" \
  "$image" -j 2 -T 900 \
  -w /opt/gigahorse/gigahorse-toolchain/.temp \
  --cache_dir /opt/gigahorse/gigahorse-toolchain/cache \
  -r /opt/gigahorse/gigahorse-toolchain/.temp/results.json \
  -C clients/visualizeout.py /input/runtime.hex >&2

abi="$work/heimdall/abi.json"
tac="$work/gigahorse-temp/runtime/out/contract.tac"
if [[ ! -s "$abi" || ! -s "$tac" ]]; then
  echo 'companion output missing: expected Heimdall abi.json and Gigahorse contract.tac' >&2
  exit 1
fi
"$engine" --bytecode-companions "$input" "$abi" "$tac" | tee "$work/analysis.json"
