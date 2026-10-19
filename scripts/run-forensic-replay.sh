#!/usr/bin/env bash
# Replay selected historical transactions locally on the VPS. No transaction is broadcast.
set -euo pipefail

if (( $# != 3 )); then
  echo "usage: $0 <forensic-report.json> <rpc-url> <output-directory>" >&2
  exit 2
fi

report=$1
rpc_url=$2
output_dir=$3
mkdir -p "$output_dir"
chmod 700 "$output_dir"
exec 9>"$output_dir/.lock"
flock -n 9 || { echo "replay already running in $output_dir" >&2; exit 3; }

for command in jq cast timeout xxd sha256sum flock; do
  command -v "$command" >/dev/null || { echo "missing $command" >&2; exit 2; }
done
jq -e '.schema == "forensic-map-v1" and .identity.exact_onchain_runtime == true and .event_history.complete_from_deployment == true' "$report" >/dev/null
chain_id=$(jq -r '.identity.chain_id' "$report")
block_number=$(jq -r '.identity.block_number' "$report")
block_hash=$(jq -r '.identity.block_hash | ascii_downcase' "$report")
address=$(jq -r '.identity.address' "$report")
runtime_sha=$(jq -r '.identity.runtime_sha256' "$report")
observed_chain=$(cast chain-id --rpc-url "$rpc_url" --rpc-timeout 30)
[[ $observed_chain == "$chain_id" ]] || { echo "chain ID mismatch" >&2; exit 4; }
block_tag=$(printf '0x%x' "$block_number")
observed_hash=$(cast rpc eth_getBlockByNumber "$block_tag" false --rpc-url "$rpc_url" --rpc-timeout 30 | jq -r '.hash | ascii_downcase')
[[ $observed_hash == "$block_hash" ]] || { echo "fixed block hash mismatch" >&2; exit 4; }
runtime_hex=$(cast code "$address" --block "$block_number" --rpc-url "$rpc_url" --rpc-timeout 30)
observed_runtime_sha=$(printf '%s' "${runtime_hex#0x}" | xxd -r -p | sha256sum | cut -d' ' -f1)
[[ $observed_runtime_sha == "$runtime_sha" ]] || { echo "fixed runtime mismatch" >&2; exit 4; }

printf 'started=%s\nchain_id=%s\nblock_number=%s\nblock_hash=%s\naddress=%s\nruntime_sha256=%s\n' \
  "$(date -u +%FT%TZ)" "$chain_id" "$block_number" "$block_hash" "$address" "$runtime_sha" >"$output_dir/provenance.txt"
printf 'This job replays historical transactions locally with cast run --no-bal; it does not broadcast transactions.\n' >>"$output_dir/provenance.txt"
: >"$output_dir/results.ndjson"

mapfile -t transaction_hashes < <(jq -r '.event_history.selected_transaction_hashes[:3][]' "$report")
for tx_hash in "${transaction_hashes[@]}"; do
  [[ $tx_hash =~ ^0x[0-9a-fA-F]{64}$ ]] || { echo "invalid transaction hash in report" >&2; exit 4; }
  trace_path="$output_dir/${tx_hash}.trace.txt"
  started=$(date -u +%FT%TZ)
  set +e
  timeout --signal=TERM --kill-after=5s 240s \
    cast run "$tx_hash" --rpc-url "$rpc_url" --rpc-timeout 30 \
      --no-bal --disable-external-identification --color never \
      >"$trace_path" 2>&1
  exit_code=$?
  set -e
  finished=$(date -u +%FT%TZ)
  trace_sha=$(sha256sum "$trace_path" | cut -d' ' -f1)
  if (( exit_code == 0 )); then status=trace_generated
  elif (( exit_code == 124 || exit_code == 137 )); then status=timeout
  else status=replay_failed
  fi
  jq -cn --arg tx "$tx_hash" --arg status "$status" --arg started "$started" \
    --arg finished "$finished" --arg path "$trace_path" --arg sha "$trace_sha" \
    --argjson exit_code "$exit_code" \
    '{transaction_hash:$tx,status:$status,exit_code:$exit_code,started:$started,finished:$finished,trace_path:$path,trace_sha256:$sha}' \
    >>"$output_dir/results.ndjson"
done
jq -s --arg completed "$(date -u +%FT%TZ)" \
  '{completed:$completed,transactions:.,trace_generated:(map(select(.status=="trace_generated"))|length),timeouts:(map(select(.status=="timeout"))|length),failures:(map(select(.status=="replay_failed"))|length)}' \
  "$output_dir/results.ndjson" >"$output_dir/summary.json.tmp"
mv "$output_dir/summary.json.tmp" "$output_dir/summary.json"
