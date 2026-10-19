#!/usr/bin/env python3
"""Snapshot-isolated token, role, and pause invariants on the pinned wCOP fork."""
import hashlib
import json
import subprocess
import sys
import time

from wcop_trace import (ANVIL, DEFAULT, RECEIVER, SLOT, TOKEN, UPSTREAM, WORK,
                        available_port, private_write, read, rpc, word_address, word_number)


def call_uint(url, selector, words=""):
    result = rpc(url, "eth_call", [{"to": TOKEN, "data": selector + words}, "latest"])
    return int(result, 16)


def send(url, sender, data):
    tx = rpc(url, "eth_sendTransaction", [{"from": sender, "to": TOKEN,
                                           "data": data, "gas": "0x989680"}])
    for _ in range(40):
        receipt = rpc(url, "eth_getTransactionReceipt", [tx])
        if receipt is not None:
            status = receipt.get("status")
            if status not in ("0x0", "0x1"):
                raise ValueError("unexpected local receipt status")
            return status == "0x1"
        time.sleep(0.1)
    raise RuntimeError("local receipt not found")


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job ID")
    job = WORK / job_id
    history, roles, engine, path_map = (read(job / n) for n in
        ("history.json", "roles.json", "engine-impl.json", "path-map.json"))
    block = int(history["through_block"])
    code_hash = hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest()
    if (history.get("address") != TOKEN or path_map.get("block") != block or
            path_map.get("engine_code_sha256") != code_hash or
            roles.get("event_replay_matches_current") is not True):
        raise ValueError("saved identity or roles mismatch")
    actors = {name: accounts[0] for name, accounts in
              (roles.get("active_accounts_from_events") or {}).items() if accounts}
    for role in ("DEFAULT_ADMIN_ROLE", "PAUSER_ROLE", "MINTER_ROLE"):
        if role not in actors:
            raise ValueError("required role holder missing")
    holder = history["funded_holder"]["address"]
    minter_hash = roles["role_hashes"]["MINTER_ROLE"].removeprefix("0x")
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    log_path = job / "tools/anvil-invariant-sequences.log"
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
            participants = {DEFAULT, holder, RECEIVER, actors["MINTER_ROLE"],
                            actors["PAUSER_ROLE"], actors["DEFAULT_ADMIN_ROLE"]}
            for account in participants:
                rpc(url, "anvil_impersonateAccount", [account])
                rpc(url, "anvil_setBalance", [account, hex(10**18)])
            has_role_data = minter_hash + word_address(RECEIVER)
            def state():
                return {"supply": call_uint(url, "0x18160ddd"),
                        "holder": call_uint(url, "0x70a08231", word_address(holder)),
                        "receiver": call_uint(url, "0x70a08231", word_address(RECEIVER)),
                        "allowance": call_uint(url, "0xdd62ed3e",
                                               word_address(holder) + word_address(RECEIVER)),
                        "paused": bool(call_uint(url, "0x5c975abb")),
                        "receiver_minter": bool(call_uint(url, "0x91d14854", has_role_data)),
                        "implementation": rpc(url, "eth_getStorageAt", [TOKEN, SLOT, "latest"])}
            baseline = state()
            if baseline["holder"] < 1 or baseline["paused"] or baseline["receiver_minter"]:
                raise ValueError("fork preconditions differ from planned scenarios")
            mint = lambda to: "0x40c10f19" + word_address(to) + word_number(1)
            transfer = "0xa9059cbb" + word_address(RECEIVER) + word_number(1)
            burn = "0x42966c68" + word_number(1)
            approve = "0x095ea7b3" + word_address(RECEIVER) + word_number(1)
            transfer_from = "0x23b872dd" + word_address(holder) + word_address(RECEIVER) + word_number(1)
            grant = "0x2f2ff15d" + minter_hash + word_address(RECEIVER)
            revoke = "0xd547741f" + minter_hash + word_address(RECEIVER)
            def scenario(name, exercise):
                snapshot = rpc(url, "evm_snapshot", [])
                before = state()
                if before != baseline:
                    raise ValueError("scenario entered with altered token state")
                try:
                    summary = exercise(before)
                    after = state()
                    if after["implementation"] != before["implementation"]:
                        raise ValueError("implementation changed in token invariant scenario")
                    cases.append({"case": name, "checks": summary,
                                  "deltas": {k: int(after[k]) - int(before[k]) for k in
                                             ("supply", "holder", "receiver", "allowance")},
                                  "paused_final": after["paused"],
                                  "receiver_minter_final": after["receiver_minter"]})
                finally:
                    if rpc(url, "evm_revert", [snapshot]) is not True:
                        raise RuntimeError("local snapshot restore failed")
                if state() != baseline:
                    raise ValueError("token state not restored after scenario")
            def unauthorized(before):
                if send(url, DEFAULT, mint(RECEIVER)):
                    raise ValueError("unauthorized mint succeeded")
                if state() != before:
                    raise ValueError("rejected mint changed token state")
                return ["unauthorized_mint_reverted", "state_unchanged"]
            scenario("unauthorized_mint", unauthorized)
            def authorized(before):
                if not send(url, actors["MINTER_ROLE"], mint(RECEIVER)):
                    raise ValueError("authorized mint reverted")
                after = state()
                if after["supply"] != before["supply"] + 1 or after["receiver"] != before["receiver"] + 1 or after["holder"] != before["holder"]:
                    raise ValueError("mint supply/balance invariant differs")
                return ["authorized_mint_succeeded", "supply_and_receiver_plus_one"]
            scenario("authorized_mint", authorized)
            def moved(before):
                if not send(url, holder, transfer):
                    raise ValueError("holder transfer reverted")
                after = state()
                if after["supply"] != before["supply"] or after["holder"] != before["holder"] - 1 or after["receiver"] != before["receiver"] + 1:
                    raise ValueError("transfer conservation invariant differs")
                return ["transfer_succeeded", "supply_constant", "balances_conserved"]
            scenario("transfer_conservation", moved)
            def burned(before):
                if not send(url, holder, burn):
                    raise ValueError("holder burn reverted")
                after = state()
                if after["supply"] != before["supply"] - 1 or after["holder"] != before["holder"] - 1 or after["receiver"] != before["receiver"]:
                    raise ValueError("burn supply/balance invariant differs")
                return ["burn_succeeded", "supply_and_holder_minus_one"]
            scenario("burn_conservation", burned)
            def paused_transfer(before):
                if not send(url, actors["PAUSER_ROLE"], "0x8456cb59") or not state()["paused"]:
                    raise ValueError("pause failed")
                paused_state = state()
                if send(url, holder, transfer) or state() != paused_state:
                    raise ValueError("paused transfer succeeded or altered state")
                if not send(url, actors["PAUSER_ROLE"], "0x3f4ba83a") or state()["paused"]:
                    raise ValueError("unpause failed")
                if not send(url, holder, transfer):
                    raise ValueError("transfer after unpause failed")
                after = state()
                if after["supply"] != before["supply"] or after["holder"] != before["holder"] - 1 or after["receiver"] != before["receiver"] + 1:
                    raise ValueError("pause sequence conservation differs")
                return ["pause_succeeded", "paused_transfer_reverted_without_state_change",
                        "unpause_succeeded", "transfer_after_unpause_succeeded"]
            scenario("pause_transfer_unpause", paused_transfer)
            def delegated_transfer(before):
                if not send(url, holder, approve) or state()["allowance"] != 1:
                    raise ValueError("approval failed")
                if not send(url, RECEIVER, transfer_from):
                    raise ValueError("delegated transfer failed")
                after = state()
                if after["allowance"] != 0 or after["supply"] != before["supply"] or after["holder"] != before["holder"] - 1 or after["receiver"] != before["receiver"] + 1:
                    raise ValueError("allowance or delegated transfer invariant differs")
                return ["approval_succeeded", "delegated_transfer_succeeded",
                        "allowance_consumed", "supply_constant"]
            scenario("approval_transfer_from", delegated_transfer)
            def role_transition(before):
                admin = actors["DEFAULT_ADMIN_ROLE"]
                if not send(url, admin, grant) or not state()["receiver_minter"]:
                    raise ValueError("grant role failed")
                if not send(url, RECEIVER, mint(RECEIVER)):
                    raise ValueError("mint after role grant failed")
                if not send(url, admin, revoke) or state()["receiver_minter"]:
                    raise ValueError("revoke role failed")
                after_revoke = state()
                if send(url, RECEIVER, mint(RECEIVER)) or state() != after_revoke:
                    raise ValueError("mint after role revoke succeeded or altered state")
                if after_revoke["supply"] != before["supply"] + 1 or after_revoke["receiver"] != before["receiver"] + 1:
                    raise ValueError("role transition supply invariant differs")
                return ["grant_enabled_mint", "revoke_disabled_mint", "one_mint_only"]
            scenario("grant_mint_revoke", role_transition)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    report = {"format": 1, "job_id": job_id, "block": block, "contract": TOKEN,
              "engine_code_sha256": code_hash, "scenarios": len(cases), "cases": cases,
              "all_snapshots_restored": True,
              "limits": ["All transactions were local to the pinned Anvil fork; none reached mainnet.",
                         "These checks cover the specific accounts, amounts and sequences listed, not all possible states.",
                         "Expected reverts are not vulnerabilities."]}
    private_write(job / "invariant-sequences.json", report)
    return {"job_id": job_id, "scenarios": len(cases), "all_snapshots_restored": True,
            "case_names": [x["case"] for x in cases]}


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_invariant_sequences.py <job-id>")
    print(json.dumps(run(sys.argv[1]), ensure_ascii=False))
