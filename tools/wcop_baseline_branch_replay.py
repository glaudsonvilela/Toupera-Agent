#!/usr/bin/env python3
"""Recover per-case conditional outcomes from a bounded replay of saved baseline cases."""
from collections import defaultdict
import hashlib
import json
import sys

from wcop_branch_replay import jump_records
from wcop_trace import WORK, private_write, read, trace_job


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job id")
    job = WORK / job_id
    saved = read(job / "trace.json")
    path_map = read(job / "path-map.json")
    if path_map["block"] != saved["block"] or path_map["engine_code_sha256"] != saved["engine_code_sha256"]:
        raise ValueError("saved map and baseline identity differ")
    source_by_pc = {int(item["pc"]): {k: item[k] for k in ("file", "line", "kind") if k in item}
                    for item in path_map["pc_locations"]}
    cases = []
    seen = defaultdict(set)
    def collect(name, group, selector, logs, failed, summary):
        records, incomplete, mismatched = jump_records(logs, source_by_pc)
        for event in records:
            seen[event["pc"]].add(event["side"])
        cases.append({"case": name, "group": group, "selector": selector,
                      "failed": failed, "jump_occurrences": sum(x["count"] for x in records),
                      "incomplete_jump_events": incomplete, "next_pc_mismatches": mismatched,
                      "jumps": records})
    replay = trace_job(job_id, capture_stack=True, on_trace=collect, write_summary=False)
    matches = (replay["cases"] == saved["cases"] and replay["pcs"] == saved["pcs"] and
               replay["observed_transitions"] == saved["observed_transitions"])
    report = {"format": 1, "job_id": job_id, "block": saved["block"],
              "engine_code_sha256": saved["engine_code_sha256"],
              "saved_trace_sha256": hashlib.sha256((job / "trace.json").read_bytes()).hexdigest(),
              "matches_saved_baseline": matches, "cases": cases,
              "observed_jumpi_pcs": len(seen),
              "both_sides_pcs": sum(len(sides) == 2 for sides in seen.values()),
              "one_side_pcs": sum(len(sides) == 1 for sides in seen.values()),
              "incomplete_jump_events": sum(c["incomplete_jump_events"] for c in cases),
              "next_pc_mismatches": sum(c["next_pc_mismatches"] for c in cases)}
    private_write(job / "baseline-branch-replay.json", report)
    if not matches:
        raise ValueError("baseline replay differs from saved trace")
    return {key: report[key] for key in ("job_id", "matches_saved_baseline", "observed_jumpi_pcs",
                                       "both_sides_pcs", "one_side_pcs", "incomplete_jump_events",
                                       "next_pc_mismatches")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_baseline_branch_replay.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
