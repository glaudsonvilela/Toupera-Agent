#!/usr/bin/env python3
"""Bounded wCOP evidence refresh followed by the local Rust forensic sieve."""
import json
from pathlib import Path
import subprocess
import sys

from wcop_trace import ROOT, WORK, private_write

ENGINE = ROOT / "target/release/wcop-forensic-sift"
STAGES = (
    "wcop_residual_guards.py",
    "wcop_clinical_guards.py",
    "wcop_residual_classify.py",
    "wcop_historical_reconcile.py",
    "wcop_invariant_sequences.py",
)


def run(job_id, refresh=False):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job ID")
    job = WORK / job_id
    if not job.is_dir() or not ENGINE.is_file():
        raise ValueError("job or built Rust engine missing")
    completed = []
    if refresh:
        for name in STAGES:
            script = ROOT / "tools" / name
            result = subprocess.run([sys.executable, str(script), job_id],
                                    capture_output=True, text=True, timeout=120)
            if result.returncode != 0:
                raise RuntimeError(name + " failed: " + result.stderr[-500:])
            completed.append(name)
    result = subprocess.run([str(ENGINE), str(job)],
                            capture_output=True, text=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError("Rust forensic sieve rejected evidence: " + result.stderr[-500:])
    report = json.loads(result.stdout)
    if report.get("job_id") != job_id:
        raise ValueError("Rust output job ID differs")
    private_write(job / "forensic-sift.json", report)
    return {"job_id": job_id, "refresh_stages": completed,
            "state": report["state"], "candidate_count": report["candidate_count"],
            "historical_events_matched": report["historical_events_matched"],
            "local_invariant_scenarios": report["local_invariant_scenarios"],
            "runtime_guard_groups": [{"id": x["id"], "pc_count": x["pc_count"]}
                                     for x in report["runtime_guard_groups"]],
            "unseen_static_jumpi": len(report["unseen_static_jumpi"])}


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3) or (len(sys.argv) == 3 and sys.argv[2] != "--refresh"):
        raise SystemExit("usage: wcop_forensic_run.py <job-id> [--refresh]")
    print(json.dumps(run(sys.argv[1], len(sys.argv) == 3), ensure_ascii=False))
