#!/usr/bin/env python3
"""Bounded residual guard probes on the pinned, local wCOP Anvil fork."""
from collections import defaultdict
import hashlib
import json
import subprocess
import sys
import time

from wcop_branch_replay import jump_records
from wcop_targeted_guards import upgrade_data
from wcop_trace import (ANVIL, DEFAULT, RECEIVER, SLOT, TOKEN, UPSTREAM, WORK,
                        available_port, private_write, read, rpc, word_address, word_number)

ZERO = "0x" + "0" * 40
WRONG_SLOT = "0x" + "00" * 18 + "beef"
REVERTING = "0x" + "00" * 18 + "beee"


def outcomes(cases):
    return {(j["pc"], j["side"]) for c in cases for j in c["jumps"]}


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
    admin = actors["DEFAULT_ADMIN_ROLE"]
    minter = actors["MINTER_ROLE"]
    upgrader = actors["UPGRADER_ROLE"]
    holder = history["funded_holder"]["address"]
    role_hash = roles["role_hashes"]["UPGRADER_ROLE"].removeprefix("0x")
    source_by_pc = {int(x["pc"]): {k: x[k] for k in ("file", "line", "kind") if k in x}
                    for x in path_map["pc_locations"]}
    prior = set()
    for n in ("baseline-branch-replay.json", "branch-replay.json", "targeted-guards.json"):
        d = read(job / n)
        if d.get("engine_code_sha256") != code_hash or d.get("block") != block:
            raise ValueError("prior replay identity mismatch")
        prior |= outcomes(d["cases"])
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    log_path = job / "tools/anvil-residual-guards.log"
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
                raise ValueError("fork runtime differs")
            # Mock targets exist only in the isolated fork; neither changes mainnet.
            rpc(url, "anvil_setCode", [WRONG_SLOT, "0x600060005260206000f3"])
            rpc(url, "anvil_setCode", [REVERTING, "0x60006000fd"])
            specs = [
                ("unpause_when_unpaused", actors["PAUSER_ROLE"], "0x3f4ba83a"),
                ("upgrade_delegatecall_approve", upgrader, upgrade_data(implementation,
                    "0x095ea7b3" + word_address(RECEIVER) + word_number(0))),
                ("transfer_to_zero", holder, "0xa9059cbb" + word_address(ZERO) + word_number(0)),
                ("transfer_zero_sender", ZERO, "0xa9059cbb" + word_address(RECEIVER) + word_number(0)),
                ("transfer_from_zero", DEFAULT, "0x23b872dd" + word_address(ZERO) + word_address(RECEIVER) + word_number(0)),
                ("transfer_from_to_zero", DEFAULT, "0x23b872dd" + word_address(holder) + word_address(ZERO) + word_number(0)),
                ("mint_to_zero", minter, "0x40c10f19" + word_address(ZERO) + word_number(0)),
                ("burn_from_zero", ZERO, "0x42966c68" + word_number(0)),
                ("approve_zero_spender", holder, "0x095ea7b3" + word_address(ZERO) + word_number(0)),
                ("approve_zero_owner", ZERO, "0x095ea7b3" + word_address(RECEIVER) + word_number(0)),
                ("grant_existing_role", admin, "0x2f2ff15d" + role_hash + word_address(upgrader)),
                ("revoke_absent_role", admin, "0xd547741f" + role_hash + word_address(RECEIVER)),
                ("upgrade_wrong_slot_local_mock", upgrader, upgrade_data(WRONG_SLOT)),
                ("upgrade_reverting_local_mock", upgrader, upgrade_data(REVERTING)),
            ]
            for name, sender, data in specs:
                call = {"from": sender, "to": TOKEN, "data": data, "value": "0x0"}
                trace = rpc(url, "debug_traceCall", [call, "latest", {"disableStorage": True,
                           "disableMemory": True, "disableStack": False}])
                expected_failure = name not in ("grant_existing_role", "revoke_absent_role", "upgrade_delegatecall_approve")
                if bool(trace.get("failed")) != expected_failure:
                    raise ValueError("unexpected result for " + name)
                jumps, incomplete, mismatched = jump_records(trace.get("structLogs") or [], source_by_pc)
                cases.append({"case": name, "selector": data[:10], "failed": bool(trace.get("failed")),
                              "implementation_steps": sum(x.get("depth") == 2 for x in trace.get("structLogs") or []),
                              "incomplete_jump_events": incomplete, "next_pc_mismatches": mismatched,
                              "jumps": jumps})
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    observed = outcomes(cases)
    seen = prior | observed
    static = {int(x["pc"]) for x in engine["instructions"] if x["mnemonic"] == "JUMPI"}
    sides = defaultdict(set)
    for pc, side in seen:
        sides[pc].add(side)
    new = sorted(observed - prior)
    report = {"format": 1, "job_id": job_id, "block": block, "contract": TOKEN,
              "engine_code_sha256": code_hash, "cases_count": len(cases),
              "prior_outcomes": len(prior), "new_outcomes": [{"pc": pc, "side": side,
                  "source": source_by_pc.get(pc)} for pc, side in new],
              "combined_outcomes": len(seen), "combined_jumpi_pcs": len(sides),
              "both_sides_pcs": sum(len(v) == 2 for v in sides.values()),
              "one_side_pcs": sum(len(v) == 1 for v in sides.values()),
              "unseen_static_jumpi_pcs": len(static - set(sides)),
              "incomplete_jump_events": sum(c["incomplete_jump_events"] for c in cases),
              "next_pc_mismatches": sum(c["next_pc_mismatches"] for c in cases),
              "cases": cases,
              "limits": ["All probes used a pinned local Anvil fork; no mainnet transaction.",
                         "Mock implementation bytecode was injected into the local fork only.",
                         "Observed EVM outcomes are not LCOV branch IDs or proof of vulnerability."]}
    private_write(job / "residual-guards.json", report)
    return {k: report[k] for k in ("job_id", "cases_count", "prior_outcomes", "new_outcomes",
                                    "combined_outcomes", "combined_jumpi_pcs", "both_sides_pcs",
                                    "one_side_pcs", "unseen_static_jumpi_pcs", "incomplete_jump_events",
                                    "next_pc_mismatches")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_residual_guards.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
