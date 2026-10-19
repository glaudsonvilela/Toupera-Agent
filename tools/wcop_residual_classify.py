#!/usr/bin/env python3
"""Classify one-sided runtime JUMPI observations by bytecode role, not LCOV ID."""
from collections import Counter, defaultdict
import hashlib
import json
import sys

from wcop_trace import WORK, private_write, read

INPUTS = ("baseline-branch-replay.json", "branch-replay.json",
          "targeted-guards.json", "residual-guards.json")


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job ID")
    job = WORK / job_id
    engine, path_map = read(job / "engine-impl.json"), read(job / "path-map.json")
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if path_map.get("engine_code_sha256") != code_hash:
        raise ValueError("path map differs from runtime")
    sides = defaultdict(set)
    cases = 0
    input_hashes = {}
    for name in INPUTS:
        path = job / name
        row = read(path)
        if row.get("engine_code_sha256") != code_hash or row.get("block") != path_map.get("block"):
            raise ValueError("replay identity mismatch: " + name)
        input_hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        cases += len(row["cases"])
        for case in row["cases"]:
            for jump in case["jumps"]:
                sides[int(jump["pc"])].add(jump["side"])
    instructions = engine["instructions"]
    index = {x["pc"]: i for i, x in enumerate(instructions)}
    source = {x["pc"]: {k: x[k] for k in ("file", "line", "kind") if k in x}
              for x in path_map["pc_locations"]}
    first_function_pc = min(x["destination_pc"]["pc"] for x in engine["selectors"])
    rows = []
    for pc, observed in sorted(sides.items()):
        if len(observed) != 1:
            continue
        i = index[pc]
        if instructions[i]["mnemonic"] != "JUMPI":
            raise ValueError("observed non-JUMPI PC")
        prev = [x["mnemonic"] for x in instructions[max(0, i - 4):i]]
        location = source.get(pc)
        if pc < first_function_pc:
            category = "dispatcher"
        elif prev == ["CALLVALUE", "DUP", "ISZERO", "PUSH"]:
            category = "nonpayable_entry"
        elif location and location["file"].endswith("#utility.yul"):
            category = "abi_utility"
        else:
            category = "runtime_guard"
        rows.append({"pc": pc, "observed_side": next(iter(observed)),
                     "category": category, "source": location})
    counts = dict(sorted(Counter(x["category"] for x in rows).items()))
    report = {"format": 1, "job_id": job_id, "block": path_map["block"],
              "engine_code_sha256": code_hash, "input_sha256": input_hashes,
              "cases_replayed": cases, "static_jumpi_pcs": sum(x["mnemonic"] == "JUMPI" for x in instructions),
              "observed_jumpi_pcs": len(sides), "both_sides_pcs": sum(len(x) == 2 for x in sides.values()),
              "one_side_pcs": len(rows), "unseen_static_jumpi_pcs":
                  sum(x["mnemonic"] == "JUMPI" for x in instructions) - len(sides),
              "one_side_categories": counts,
              "rows": rows,
              "limits": ["Bytecode-pattern categories are structural triage, not reachability proofs.",
                         "One-sided JUMPI observations are not LCOV branch IDs.",
                         "Runtime guards may need a different legitimate state to exercise the other side."]}
    if len(rows) != sum(counts.values()) or len(sides) != report["both_sides_pcs"] + len(rows):
        raise ValueError("classification count mismatch")
    private_write(job / "residual-classification.json", report)
    return {k: report[k] for k in ("job_id", "cases_replayed", "static_jumpi_pcs",
        "observed_jumpi_pcs", "both_sides_pcs", "one_side_pcs", "unseen_static_jumpi_pcs",
        "one_side_categories")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_residual_classify.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
