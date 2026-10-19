#!/usr/bin/env python3
"""Pin context and error selectors for direct implementation and proxy permit guards."""
import hashlib
import json
import subprocess
import sys
import time

from wcop_branch_replay import jump_records
from wcop_targeted_guards import initialize_data, permit_data, upgrade_data
from wcop_trace import (ANVIL, DEFAULT, RECEIVER, SLOT, TOKEN, UPSTREAM, WORK,
                        available_port, private_write, read, rpc)

ERRORS = {"ECDSAInvalidSignature": "f645eedf",
          "ECDSAInvalidSignatureS": "d78bce0c",
          "ERC2612InvalidSigner": "4b800e46",
          "InvalidInitialization": "f92ee8a9",
          "UUPSUnauthorizedCallContext": "e07c8dba"}
WATCH = {2507, 2533, 2548, 2557, 4145, 4235, 4540,
         5891, 5972, 6001, 6051, 6064, 6104, 6117}


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job ID")
    job = WORK / job_id
    history, engine, roles, path_map = (read(job / n) for n in
        ("history.json", "engine-impl.json", "roles.json", "path-map.json"))
    block = int(history["through_block"])
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if (history.get("address") != TOKEN or path_map.get("block") != block or
            path_map.get("engine_code_sha256") != code_hash):
        raise ValueError("saved identity mismatch")
    actors = {name: accounts[0] for name, accounts in
              (roles.get("active_accounts_from_events") or {}).items() if accounts}
    for role in ("DEFAULT_ADMIN_ROLE", "PAUSER_ROLE", "MINTER_ROLE", "UPGRADER_ROLE"):
        if role not in actors:
            raise ValueError("missing actor: " + role)
    holder = history["funded_holder"]["address"]
    source_by_pc = {int(x["pc"]): {k: x[k] for k in ("file", "line", "kind") if k in x}
                    for x in path_map["pc_locations"]}
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    log_path = job / "tools/anvil-clinical-guards.log"
    cases = []
    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen([ANVIL, "--fork-url", UPSTREAM, "--fork-block-number", str(block),
                                    "--host", "127.0.0.1", "--port", str(port), "--silent"],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(30):
                if process.poll() is not None:
                    raise RuntimeError("Anvil stopped before ready")
                try:
                    if int(rpc(url, "eth_chainId", []), 16) == 1:
                        break
                except (OSError, ValueError):
                    time.sleep(0.4)
            else:
                raise RuntimeError("Anvil did not become ready")
            word = rpc(url, "eth_getStorageAt", [TOKEN, SLOT, "latest"])
            implementation = "0x" + word[-40:].lower()
            if rpc(url, "eth_getCode", [implementation, "latest"]).lower() != (job / "implementation.hex").read_text().strip().lower():
                raise ValueError("fork runtime differs from saved bytecode")
            specs = [
                ("implementation_proxiable_uuid", implementation, DEFAULT, "0x52d1902d", False, None, 1),
                ("implementation_upgrade_direct", implementation, actors["UPGRADER_ROLE"],
                 upgrade_data(implementation), True, "UUPSUnauthorizedCallContext", 1),
                ("implementation_initialize_direct", implementation, actors["DEFAULT_ADMIN_ROLE"],
                 initialize_data(actors), True, "InvalidInitialization", 1),
                ("proxy_permit_zero_rs", TOKEN, DEFAULT, permit_data(holder, 2**256 - 1, 27, 0, 0),
                 True, "ECDSAInvalidSignature", 2),
                ("proxy_permit_bad_v", TOKEN, DEFAULT, permit_data(holder, 2**256 - 1, 29, 1, 1),
                 True, "ECDSAInvalidSignature", 2),
                ("proxy_permit_wrong_signer", TOKEN, DEFAULT, permit_data(holder, 2**256 - 1, 27, 1, 1),
                 True, "ERC2612InvalidSigner", 2),
                ("proxy_permit_high_s", TOKEN, DEFAULT, permit_data(holder, 2**256 - 1, 27, 1, 2**255),
                 True, "ECDSAInvalidSignatureS", 2),
            ]
            for name, target, sender, data, expected_failure, expected_error, depth in specs:
                trace = rpc(url, "debug_traceCall", [{"from": sender, "to": target, "data": data,
                    "value": "0x0"}, "latest", {"disableStorage": True,
                    "disableMemory": True, "disableStack": False}])
                failed = bool(trace.get("failed"))
                result = trace.get("returnValue", "").removeprefix("0x").lower()
                selector = result[:8] if failed else None
                if failed != expected_failure:
                    raise ValueError("unexpected completion: " + name)
                if expected_error and selector != ERRORS[expected_error]:
                    raise ValueError("unexpected revert selector for " + name + ": " + selector)
                if name == "implementation_proxiable_uuid" and result != SLOT.removeprefix("0x"):
                    raise ValueError("direct proxiable UUID differs from ERC-1967 slot")
                jumps, incomplete, mismatched = jump_records(trace.get("structLogs") or [], source_by_pc, depth)
                if incomplete or mismatched:
                    raise ValueError("incomplete or inconsistent jump trace: " + name)
                cases.append({"case": name, "context": "implementation_direct" if depth == 1 else "proxy_delegatecall",
                              "failed": failed, "return_selector": selector,
                              "expected_error": expected_error, "jump_occurrences": sum(x["count"] for x in jumps),
                              "watched_jumps": [x for x in jumps if x["pc"] in WATCH],
                              "incomplete_jump_events": incomplete, "next_pc_mismatches": mismatched})
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    report = {"format": 1, "job_id": job_id, "block": block, "contract": TOKEN,
              "engine_code_sha256": code_hash, "cases_count": len(cases), "cases": cases,
              "limits": ["Calls ran only on a pinned Anvil fork; no mainnet transaction.",
                         "Depth-1 direct implementation traces are separate from depth-2 proxy traces.",
                         "A local revert selector confirms the measured error, not global reachability."]}
    private_write(job / "clinical-guards.json", report)
    return {"job_id": job_id, "cases": [{k: x[k] for k in ("case", "context", "failed",
            "return_selector", "watched_jumps")} for x in cases]}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_clinical_guards.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
