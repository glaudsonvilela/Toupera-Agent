#!/usr/bin/env python3
"""Check unobserved initialization and UUPS guards on one pinned local Anvil fork."""
from collections import Counter
import hashlib
import json
import subprocess
import sys
import time

from wcop_branch_replay import jump_records
from wcop_trace import (ANVIL, DEFAULT, RECEIVER, SLOT, TOKEN, UPSTREAM, WORK,
                        available_port, private_write, read, rpc, word_address, word_number)


def initialize_data(actors):
    # Six ABI heads: four addresses, followed by offsets to two empty strings.
    return ("0x9317c200" + "".join(word_address(actors[name]) for name in
            ("DEFAULT_ADMIN_ROLE", "PAUSER_ROLE", "MINTER_ROLE", "UPGRADER_ROLE"))
            + word_number(192) + word_number(224) + word_number(0) + word_number(0))


def upgrade_data(implementation, data="0x"):
    raw = bytes.fromhex(data.removeprefix("0x"))
    return ("0x4f1ef286" + word_address(implementation) + word_number(64)
            + word_number(len(raw)) + raw.hex().ljust(((len(raw) + 31) // 32) * 64, "0"))


def permit_data(owner, deadline, v, r, s):
    return ("0xd505accf" + word_address(owner) + word_address(RECEIVER)
            + word_number(1) + word_number(deadline) + word_number(v)
            + word_number(r) + word_number(s))


def outcome_set(cases):
    return {(event["pc"], event["side"]) for case in cases for event in case["jumps"]}


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job id")
    job = WORK / job_id
    history, engine, roles, path_map, baseline, adaptive = (read(job / name) for name in
        ("history.json", "engine-impl.json", "roles.json", "path-map.json",
         "baseline-branch-replay.json", "branch-replay.json"))
    block = int(history["through_block"])
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if (history.get("address") != TOKEN or path_map.get("block") != block or
            path_map.get("engine_code_sha256") != code_hash or
            baseline.get("block") != block or baseline.get("engine_code_sha256") != code_hash or
            adaptive.get("block") != block or adaptive.get("engine_code_sha256") != code_hash):
        raise ValueError("saved identity mismatch")
    active = roles.get("active_accounts_from_events") or {}
    actors = {name: accounts[0] for name, accounts in active.items() if accounts}
    required = ("DEFAULT_ADMIN_ROLE", "PAUSER_ROLE", "MINTER_ROLE", "UPGRADER_ROLE")
    if any(name not in actors for name in required):
        raise ValueError("required role account not observed")
    role_hash = roles["role_hashes"]["UPGRADER_ROLE"].removeprefix("0x")
    holder = (history.get("funded_holder") or {}).get("address")
    if not holder:
        raise ValueError("saved funded holder missing")
    source_by_pc = {int(item["pc"]): {key: item[key] for key in ("file", "line", "kind") if key in item}
                    for item in path_map["pc_locations"]}
    known = outcome_set(baseline["cases"]) | outcome_set(adaptive["cases"])
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    log_path = job / "tools/anvil-targeted-guards.log"
    cases = []
    with log_path.open("w", encoding="utf-8") as log:
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
                raise ValueError("fork bytecode differs from saved implementation")
            rpc(url, "anvil_setBalance", [DEFAULT, hex(10**18)])
            rpc(url, "anvil_setBalance", [actors["UPGRADER_ROLE"], hex(10**18)])
            init = initialize_data(actors)
            specs = [
                ("initialize_valid_already_initialized", actors["DEFAULT_ADMIN_ROLE"], init, 0),
                ("initialize_nonpayable_value", DEFAULT, init, 1),
                ("upgrade_unauthorized", DEFAULT, upgrade_data(implementation), 0),
                ("upgrade_same_implementation", actors["UPGRADER_ROLE"], upgrade_data(implementation), 0),
                ("upgrade_invalid_implementation", actors["UPGRADER_ROLE"], upgrade_data("0x" + "0" * 40), 0),
                ("upgrade_delegatecall_name", actors["UPGRADER_ROLE"], upgrade_data(implementation, "0x06fdde03"), 0),
                ("upgrade_delegatecall_unknown", actors["UPGRADER_ROLE"], upgrade_data(implementation, "0xdeadbeef"), 0),
                ("upgrade_nonpayable_value", actors["UPGRADER_ROLE"], upgrade_data(implementation), 1),
                ("proxiable_uuid_via_proxy", DEFAULT, "0x52d1902d", 0),
                ("has_role_upgrader", DEFAULT, "0x91d14854" + role_hash + word_address(actors["UPGRADER_ROLE"]), 0),
                ("get_role_admin", DEFAULT, "0x248a9ca3" + role_hash, 0),
                ("renounce_role_self", actors["UPGRADER_ROLE"], "0x36568abe" + role_hash + word_address(actors["UPGRADER_ROLE"]), 0),
                ("renounce_role_mismatch", DEFAULT, "0x36568abe" + role_hash + word_address(actors["UPGRADER_ROLE"]), 0),
                ("supports_access_control", DEFAULT, "0x01ffc9a7" + "7965db0b".ljust(64, "0"), 0),
                ("supports_erc165", DEFAULT, "0x01ffc9a7" + "01ffc9a7".ljust(64, "0"), 0),
                ("supports_unknown", DEFAULT, "0x01ffc9a7" + "ffffffff".ljust(64, "0"), 0),
                ("allowance_holder", DEFAULT, "0xdd62ed3e" + word_address(holder) + word_address(RECEIVER), 0),
                ("nonces_holder", DEFAULT, "0x7ecebe00" + word_address(holder), 0),
                ("permit_expired", DEFAULT, permit_data(holder, 0, 27, 1, 1), 0),
                ("permit_high_s", DEFAULT, permit_data(holder, 2**256 - 1, 27, 1, 2**255), 0),
                ("permit_bad_v", DEFAULT, permit_data(holder, 2**256 - 1, 29, 1, 1), 0),
            ]
            # Sign a valid EIP-2612 permit with an ephemeral Anvil development account.
            owner = rpc(url, "eth_accounts", [])[0]
            name_result = bytes.fromhex(rpc(url, "eth_call", [{"to": TOKEN, "data": "0x06fdde03"}, "latest"])[2:])
            name_length = int.from_bytes(name_result[32:64], "big")
            token_name = name_result[64:64 + name_length].decode("utf-8")
            nonce = int(rpc(url, "eth_call", [{"to": TOKEN, "data": "0x7ecebe00" + word_address(owner)}, "latest"]), 16)
            deadline = 2**256 - 1
            typed = {"types": {"EIP712Domain": [
                        {"name": "name", "type": "string"}, {"name": "version", "type": "string"},
                        {"name": "chainId", "type": "uint256"}, {"name": "verifyingContract", "type": "address"}],
                       "Permit": [
                        {"name": "owner", "type": "address"}, {"name": "spender", "type": "address"},
                        {"name": "value", "type": "uint256"}, {"name": "nonce", "type": "uint256"},
                        {"name": "deadline", "type": "uint256"}]},
                     "primaryType": "Permit",
                     "domain": {"name": token_name, "version": "1", "chainId": 1,
                                "verifyingContract": TOKEN},
                     "message": {"owner": owner, "spender": RECEIVER, "value": "1",
                                 "nonce": str(nonce), "deadline": str(deadline)}}
            signature = bytes.fromhex(rpc(url, "eth_signTypedData_v4", [owner, json.dumps(typed)])[2:])
            if len(signature) != 65:
                raise ValueError("unexpected local signature length")
            r = int.from_bytes(signature[:32], "big")
            sig_s = int.from_bytes(signature[32:64], "big")
            v = signature[64]
            specs.append(("permit_valid_local_signer", owner,
                          permit_data(owner, deadline, v, r, sig_s), 0))
            for name, sender, data, value in specs:
                call = {"from": sender, "to": TOKEN, "data": data, "value": hex(value)}
                trace = rpc(url, "debug_traceCall", [call, "latest", {"disableStorage": True,
                            "disableMemory": True, "disableStack": False}])
                records, incomplete, mismatched = jump_records(trace.get("structLogs") or [], source_by_pc)
                cases.append({"case": name, "selector": data[:10], "failed": bool(trace.get("failed")),
                              "implementation_steps": sum(row.get("depth") == 2 for row in trace.get("structLogs") or []),
                              "jump_occurrences": sum(row["count"] for row in records),
                              "incomplete_jump_events": incomplete,
                              "next_pc_mismatches": mismatched, "jumps": records})
                if name == "permit_valid_local_signer":
                    if cases[-1]["failed"]:
                        raise ValueError("signed local permit trace failed")
                    snapshot = rpc(url, "evm_snapshot", [])
                    try:
                        tx = rpc(url, "eth_sendTransaction", [{"from": owner, "to": TOKEN,
                                     "data": data, "gas": "0x989680"}])
                        receipt = None
                        for _ in range(30):
                            receipt = rpc(url, "eth_getTransactionReceipt", [tx])
                            if receipt is not None:
                                break
                            time.sleep(0.1)
                        if receipt is None or receipt.get("status") != "0x1":
                            raise ValueError("signed local permit transaction failed")
                        next_nonce = int(rpc(url, "eth_call", [{"to": TOKEN,
                            "data": "0x7ecebe00" + word_address(owner)}, "latest"]), 16)
                        allowance = int(rpc(url, "eth_call", [{"to": TOKEN,
                            "data": "0xdd62ed3e" + word_address(owner) + word_address(RECEIVER)}, "latest"]), 16)
                        if next_nonce != nonce + 1 or allowance != 1:
                            raise ValueError("signed local permit state differs")
                        replay = rpc(url, "debug_traceCall", [call, "latest",
                            {"disableStorage": True, "disableMemory": True, "disableStack": True}])
                        if not replay.get("failed"):
                            raise ValueError("signed local permit accepted a second time")
                        replay_nonce = int(rpc(url, "eth_call", [{"to": TOKEN,
                            "data": "0x7ecebe00" + word_address(owner)}, "latest"]), 16)
                        replay_allowance = int(rpc(url, "eth_call", [{"to": TOKEN,
                            "data": "0xdd62ed3e" + word_address(owner) + word_address(RECEIVER)}, "latest"]), 16)
                        if replay_nonce != next_nonce or replay_allowance != allowance:
                            raise ValueError("rejected local replay altered state")
                        cases[-1]["local_state_replay"] = {"receipt_status": "success",
                            "nonce_delta": next_nonce - nonce, "allowance": allowance,
                            "same_signature_replay_failed": True, "state_unchanged_after_replay": True}
                    finally:
                        if rpc(url, "evm_revert", [snapshot]) is not True:
                            raise RuntimeError("local permit snapshot restore failed")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    observed = outcome_set(cases)
    delta = sorted(observed - known)
    static_jumpi = {int(item["pc"]) for item in engine["instructions"] if item["mnemonic"] == "JUMPI"}
    report = {"format": 1, "job_id": job_id, "block": block, "contract": TOKEN,
              "engine_code_sha256": code_hash, "baseline_outcomes": len(known),
              "targeted_cases": len(cases), "targeted_outcomes": len(observed),
              "new_outcomes": [{"pc": pc, "side": side, "source": source_by_pc.get(pc)} for pc, side in delta],
              "combined_outcomes": len(known | observed),
              "combined_jumpi_pcs": len({pc for pc, side in known | observed}),
              "unseen_static_jumpi_pcs": len(static_jumpi - {pc for pc, side in known | observed}),
              "incomplete_jump_events": sum(case["incomplete_jump_events"] for case in cases),
              "next_pc_mismatches": sum(case["next_pc_mismatches"] for case in cases),
              "cases": cases,
              "limits": ["All calls ran on a pinned local Anvil fork and made no mainnet transaction.",
                         "A failed call can still reveal dispatcher and guard paths; it does not prove a vulnerability.",
                         "Runtime jump outcomes are not LCOV branch IDs."]}
    private_write(job / "targeted-guards.json", report)
    return {key: report[key] for key in ("job_id", "targeted_cases", "baseline_outcomes",
                                       "combined_outcomes", "combined_jumpi_pcs",
                                       "unseen_static_jumpi_pcs", "new_outcomes",
                                       "incomplete_jump_events", "next_pc_mismatches")}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_targeted_guards.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
