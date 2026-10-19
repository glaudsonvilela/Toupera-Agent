#!/usr/bin/env python3
"""Bounded, private replay of saved wCOP cases on an identity-checked Anvil fork."""
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

from wcop_adaptive import evaluate_item
from wcop_trace import (ANVIL, UPSTREAM, TOKEN, SLOT, WORK, available_port,
                        private_write, read, rpc)

EXTRA_LIMIT = 65


def jump_records(logs, source_by_pc):
    counts = Counter()
    incomplete = 0
    mismatched = 0
    for i, row in enumerate(logs):
        if row.get("depth") != 2 or row.get("op") != "JUMPI":
            continue
        stack = row.get("stack") or []
        if len(stack) < 2:
            incomplete += 1
            continue
        pc = int(row["pc"])
        destination = int(stack[-1], 16)
        side = "taken" if int(stack[-2], 16) else "not_taken"
        next_pc = int(logs[i + 1]["pc"]) if i + 1 < len(logs) and logs[i + 1].get("depth") == 2 else None
        expected = destination if side == "taken" else pc + 1
        if next_pc is None:
            incomplete += 1
        elif next_pc != expected:
            mismatched += 1
        counts[(pc, side, destination, next_pc)] += 1
    records = []
    for (pc, side, destination, next_pc), count in sorted(counts.items()):
        row = {"pc": pc, "side": side, "destination": destination,
               "next_pc": next_pc, "count": count}
        if pc in source_by_pc:
            row["source"] = source_by_pc[pc]
        records.append(row)
    return records, incomplete, mismatched


def extra_candidates(candidates, attempted):
    remaining = [item for item in candidates if item["id"] not in attempted]
    groups = defaultdict(list)
    for item in remaining:
        groups[item["family"]].append(item)
    for group in groups.values():
        group.sort(key=lambda x: (not bool(x.get("boundary")),
                                  int(x.get("amount", 0)) not in (0, 1), x["id"]))
    chosen = []
    while len(chosen) < EXTRA_LIMIT and any(groups.values()):
        for family in sorted(groups):
            if groups[family] and len(chosen) < EXTRA_LIMIT:
                chosen.append(groups[family].pop(0))
    return chosen


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job id")
    job = WORK / job_id
    history, engine, baseline, state, path_map = (read(job / name) for name in
        ("history.json", "engine-impl.json", "trace.json", "adaptive-state.json", "path-map.json"))
    block = int(history["through_block"])
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if (history.get("address") != TOKEN or baseline.get("block") != block or
            baseline.get("engine_code_sha256") != code_hash or
            state.get("block") != block or state.get("engine_code_sha256") != code_hash or
            path_map.get("block") != block or path_map.get("engine_code_sha256") != code_hash):
        raise ValueError("saved job identity differs")
    by_id = {item["id"]: item for item in state["candidates"]}
    selected = [by_id[row["id"]] for row in state["attempts"]]
    if len(selected) != len(state["attempts"]):
        raise ValueError("saved attempts differ")
    extra = extra_candidates(state["candidates"], {item["id"] for item in selected})
    instructions = {int(item["pc"]) for item in engine["instructions"]}
    block_for = {int(pc): item["id"] for item in engine["blocks"] for pc in item["instruction_pcs"]}
    source_by_pc = {int(item["pc"]): {k: item[k] for k in ("file", "line", "kind") if k in item}
                    for item in path_map["pc_locations"]}
    static_jumpi = {int(item["pc"]) for item in engine["instructions"] if item["mnemonic"] == "JUMPI"}
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    results = []
    seen = defaultdict(set)
    failures = []
    anvil_log = job / "tools/anvil-branch-replay.log"
    with anvil_log.open("w", encoding="utf-8") as log:
        process = subprocess.Popen([ANVIL, "--fork-url", UPSTREAM, "--fork-block-number", str(block),
                                    "--host", "127.0.0.1", "--port", str(port), "--silent"],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(30):
                if process.poll() is not None:
                    raise RuntimeError("Anvil ended before ready")
                try:
                    if int(rpc(url, "eth_chainId", []), 16) == 1:
                        break
                except (OSError, ValueError):
                    time.sleep(0.4)
            else:
                raise RuntimeError("Anvil did not become ready")
            word = rpc(url, "eth_getStorageAt", [TOKEN, SLOT, "latest"])
            implementation = "0x" + word[-40:].lower()
            recorded = (job / "implementation.hex").read_text().strip()
            if rpc(url, "eth_getCode", [implementation, "latest"]).lower() != recorded.lower():
                raise ValueError("fork bytecode differs from saved runtime")
            for sender in {item["sender"] for item in selected + extra}:
                rpc(url, "anvil_impersonateAccount", [sender])
                rpc(url, "anvil_setBalance", [sender, hex(10**18)])
            for phase, items in (("saved_rust_selection", selected), ("new_boundary_cases", extra)):
                for item in items:
                    snapshot = rpc(url, "evm_snapshot", []) if phase == "new_boundary_cases" else None
                    try:
                        measured = evaluate_item(url, item, instructions, block_for, capture_stack=True)
                        records, incomplete, mismatched = jump_records(measured["logs"], source_by_pc)
                        row = {"candidate_id": item["id"], "phase": phase, "family": item["family"],
                               "selector": item["selector"], "amount": item.get("amount"),
                               "setup_ok": measured["setup_ok"], "failed": measured["failed"],
                               "committed_locally": measured["committed"],
                               "discrepancy": measured["discrepancy"],
                               "repeatable_discrepancy": measured["repeatable"],
                               "jump_occurrences": sum(x["count"] for x in records),
                               "incomplete_jump_events": incomplete,
                               "next_pc_mismatches": mismatched, "jumps": records}
                        if phase == "saved_rust_selection":
                            prior = state["attempts"][len(results)]
                            row["matches_saved_result"] = (row["failed"] == prior["failed"] and
                                                           row["setup_ok"] == prior["setup_ok"])
                        for event in records:
                            seen[event["pc"]].add(event["side"])
                        results.append(row)
                        if mismatched or incomplete or row["discrepancy"]:
                            failures.append({"candidate_id": item["id"], "mismatched": mismatched,
                                             "incomplete": incomplete,
                                             "discrepancy": row["discrepancy"]})
                    finally:
                        if snapshot is not None and rpc(url, "evm_revert", [snapshot]) is not True:
                            raise RuntimeError("local snapshot restore failed")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    report = {"format": 1, "job_id": job_id, "contract": TOKEN, "block": block,
              "engine_code_sha256": code_hash,
              "adaptive_state_sha256": hashlib.sha256((job / "adaptive-state.json").read_bytes()).hexdigest(),
              "scope": "local Anvil fork; no mainnet transactions; saved Rust selections plus bounded boundary cases",
              "saved_cases": len(selected), "new_cases": len(extra), "static_jumpi_pcs": len(static_jumpi),
              "observed_jumpi_pcs": len(seen),
              "both_sides_pcs": sum(len(sides) == 2 for sides in seen.values()),
              "one_side_pcs": sum(len(sides) == 1 for sides in seen.values()),
              "one_side_pc_list": sorted(pc for pc, sides in seen.items() if len(sides) == 1),
              "saved_case_mismatches": sum(row.get("matches_saved_result") is False for row in results),
              "review_items": failures, "cases": results,
              "limits": ["Source line matches do not equate an EVM outcome to an LCOV branch ID.",
                         "Unseen JUMPI sides have not been shown reachable.",
                         "Exploratory cases start after the saved local selection and are snapshot-isolated."]}
    private_write(job / "branch-replay.json", report)
    return {key: report[key] for key in ("job_id", "saved_cases", "new_cases", "static_jumpi_pcs",
                                       "observed_jumpi_pcs", "both_sides_pcs", "one_side_pcs",
                                       "saved_case_mismatches")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_branch_replay.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
