#!/usr/bin/env python3
"""Bounded, feedback-guided wCOP exploration on a pinned local Anvil fork."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
from wcop_trace import (ROOT, WORK, ANVIL, UPSTREAM, TOKEN, DEFAULT, RECEIVER,
                        SECOND, SLOT, available_port, private_write, read, rpc,
                        word_address, word_number)

ENGINE = ROOT / "target/release/security-engine"
MAX_ATTEMPTS = 32


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def call(url, data, sender=DEFAULT):
    return rpc(url, "eth_call", [{"from": sender, "to": TOKEN, "data": data}, "latest"])


def wait_receipt(url, tx, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        receipt = rpc(url, "eth_getTransactionReceipt", [tx])
        if receipt is not None:
            return receipt
        time.sleep(0.1)
    raise TimeoutError("a cópia local não confirmou a operação no prazo")


def uint(url, data):
    return int(call(url, data), 16)


def balance(url, address):
    return uint(url, "0x70a08231" + word_address(address))


def view(url, item):
    kind = item["family"]
    sender = item["sender"]
    recipient = item.get("recipient")
    owner = item.get("owner", sender)
    result = {"supply": uint(url, "0x18160ddd")}
    if kind in ("transfer", "transferFrom", "mint", "burn"):
        result["owner_balance"] = balance(url, owner)
        if recipient and recipient.lower() != owner.lower():
            result["recipient_balance"] = balance(url, recipient)
    if kind == "approve":
        result["allowance"] = uint(url, "0xdd62ed3e" + word_address(sender) + word_address(recipient))
    if kind in ("pause", "unpause"):
        result["paused"] = uint(url, "0x5c975abb") != 0
    if kind in ("grant", "revoke"):
        result["has_role"] = uint(url, "0x91d14854" + item["role_hash"].removeprefix("0x")
                                  + word_address(recipient)) != 0
    return result


def deviations(item, before, after):
    kind = item["family"]
    amount = int(item.get("amount", 0))
    errors = []
    expected_supply = amount if kind == "mint" else -amount if kind == "burn" else 0
    if after["supply"] - before["supply"] != expected_supply:
        errors.append("supply_delta")
    if kind in ("transfer", "transferFrom", "mint", "burn"):
        expected_owner = (-amount if kind in ("transfer", "transferFrom", "burn") else 0)
        if after["owner_balance"] - before["owner_balance"] != expected_owner:
            errors.append("owner_balance_delta")
        if kind != "burn" and item.get("recipient", "").lower() != item.get("owner", item["sender"]).lower():
            if after.get("recipient_balance", 0) - before.get("recipient_balance", 0) != amount:
                errors.append("recipient_balance_delta")
    if kind == "approve" and after.get("allowance") != amount:
        errors.append("allowance_value")
    if kind in ("pause", "unpause") and after.get("paused") != (kind == "pause"):
        errors.append("pause_state")
    if kind in ("grant", "revoke") and after.get("has_role") != (kind == "grant"):
        errors.append("role_state")
    return errors


def candidates(holder, actors, holder_balance, supply, roles):
    values = sorted({0, 1, 2, 11, min(holder_balance, 10**24),
                     min(holder_balance + 1, 10**24 + 1), min(supply + 1, 10**30)})
    values = [v for v in values if 0 <= v < 2**256]
    minter = actors.get("MINTER_ROLE")
    pauser = actors.get("PAUSER_ROLE")
    admin = actors.get("DEFAULT_ADMIN_ROLE")
    role_hash = (roles.get("role_hashes") or {}).get("MINTER_ROLE")
    out = []
    def add(family, selector, data, sender, amount=0, recipient=None, owner=None, role=None):
        if not sender:
            return
        base = {"family": family, "selector": selector, "data": data, "sender": sender,
                "amount": str(amount), "boundary": amount in (0, min(holder_balance + 1, 10**24 + 1))}
        if recipient: base["recipient"] = recipient
        if owner: base["owner"] = owner
        if role: base["role_hash"] = role
        key = json.dumps(base, sort_keys=True).encode()
        base["id"] = digest(key)[:16]
        out.append(base)
    for amount in values:
        if holder:
            for sender in (holder, RECEIVER):
                add("transfer", "0xa9059cbb", "0xa9059cbb" + word_address(SECOND) + word_number(amount),
                    sender, amount, SECOND, sender)
                add("burn", "0x42966c68", "0x42966c68" + word_number(amount), sender, amount, None, sender)
            add("approve", "0x095ea7b3", "0x095ea7b3" + word_address(SECOND) + word_number(amount),
                holder, amount, SECOND)
            add("transferFrom", "0x23b872dd", "0x23b872dd" + word_address(holder)
                + word_address(RECEIVER) + word_number(amount), SECOND, amount, RECEIVER, holder)
        for sender in (minter, SECOND, DEFAULT):
            add("mint", "0x40c10f19", "0x40c10f19" + word_address(RECEIVER) + word_number(amount),
                sender, amount, RECEIVER, sender)
    for sender in (pauser, DEFAULT):
        add("pause", "0x8456cb59", "0x8456cb59", sender)
        add("unpause", "0x3f4ba83a", "0x3f4ba83a", sender)
    if role_hash:
        for sender in (admin, DEFAULT):
            add("grant", "0x2f2ff15d", "0x2f2ff15d" + role_hash.removeprefix("0x")
                + word_address(SECOND), sender, recipient=SECOND, role=role_hash)
            add("revoke", "0xd547741f", "0xd547741f" + role_hash.removeprefix("0x")
                + word_address(SECOND), sender, recipient=SECOND, role=role_hash)
    return list({item["id"]: item for item in out}.values())[:256]


def related_values(item):
    amount = int(item.get("amount", 0))
    if item["family"] not in ("transfer", "transferFrom", "mint", "burn", "approve"):
        return []
    choices = {max(0, amount - 1), min(10**30, amount + 1),
               min(10**30, amount * 2 + 1)}
    return sorted(choices - {amount})


def neighbor(item, amount):
    family = item["family"]
    sender = item["sender"]
    recipient = item.get("recipient")
    owner = item.get("owner", sender)
    prefix = item["selector"]
    if family == "transfer":
        data = prefix + word_address(recipient) + word_number(amount)
    elif family == "transferFrom":
        data = prefix + word_address(owner) + word_address(recipient) + word_number(amount)
    elif family in ("mint", "approve"):
        data = prefix + word_address(recipient) + word_number(amount)
    elif family == "burn":
        data = prefix + word_number(amount)
    else:
        return None
    result = dict(item, data=data, amount=str(amount), boundary=True)
    result.pop("id", None)
    result["id"] = digest(json.dumps(result, sort_keys=True).encode())[:16]
    return result


def grow_candidates(state, item, new_pcs, new_edges, deviation):
    if not (new_pcs or new_edges or deviation):
        return
    known = {candidate["id"] for candidate in state["candidates"]}
    for amount in related_values(item):
        if len(state["candidates"]) >= 256:
            break
        next_item = neighbor(item, amount)
        if next_item and next_item["id"] not in known:
            state["candidates"].append(next_item)
            known.add(next_item["id"])


def run(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    history, roles, engine, baseline = (read(job / name) for name in
                                        ("history.json", "roles.json", "engine-impl.json", "trace.json"))
    status = read(job / "status.json")
    if status.get("state") != "running" or history.get("address") != TOKEN:
        raise ValueError("trabalho ativo do contrato esperado necessário")
    block = int(history["through_block"])
    if baseline.get("block") != block:
        raise ValueError("leitura anterior de outro bloco")
    code_hash = digest(bytes.fromhex(engine["normalized_hex"].removeprefix("0x")))
    if baseline.get("engine_code_sha256") != code_hash:
        raise ValueError("runtime diferente da leitura anterior")
    instructions = {int(x["pc"]) for x in engine["instructions"]}
    block_for = {int(pc): b["id"] for b in engine["blocks"] for pc in b["instruction_pcs"]}
    uncertain = {int(x["location"]["Bytecode"]["pc"]) for x in engine["unresolved"]
                 if isinstance(x.get("location", {}).get("Bytecode"), dict)
                 and isinstance(x["location"]["Bytecode"].get("pc"), int)}
    active = roles.get("active_accounts_from_events") or {}
    actors = {key: (value[0] if value else None) for key, value in active.items()}
    holder = (history.get("funded_holder") or {}).get("address")
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    anvil_log = job / "tools/anvil-adaptive.log"
    with anvil_log.open("w", encoding="utf-8") as log:
        process = subprocess.Popen([ANVIL, "--fork-url", UPSTREAM, "--fork-block-number", str(block),
                                    "--host", "127.0.0.1", "--port", str(port), "--silent"],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT)
        try:
            for _ in range(30):
                if process.poll() is not None:
                    raise RuntimeError("cópia local terminou")
                try:
                    if int(rpc(url, "eth_chainId", []), 16) == 1:
                        break
                except (OSError, ValueError):
                    time.sleep(0.4)
            else:
                raise RuntimeError("cópia local não respondeu")
            word = rpc(url, "eth_getStorageAt", [TOKEN, SLOT, "latest"])
            implementation = "0x" + word[-40:].lower()
            recorded = (job / "implementation.hex").read_text().strip()
            if rpc(url, "eth_getCode", [implementation, "latest"]).lower() != recorded.lower():
                raise ValueError("runtime local diverge do bloco")
            holder_balance = balance(url, holder) if holder else 0
            supply = uint(url, "0x18160ddd")
            pool = candidates(holder, actors, holder_balance, supply, roles)
            for sender in {x["sender"] for x in pool}:
                rpc(url, "anvil_impersonateAccount", [sender])
                rpc(url, "anvil_setBalance", [sender, hex(10**18)])
            state = {"format": 1, "address": TOKEN, "block": block,
                     "engine_code_sha256": code_hash, "candidates": pool,
                     "baseline_pcs": sorted(set(baseline["pcs"])),
                     "visited_pcs": sorted(set(baseline["pcs"])),
                     "visited_edges": baseline["observed_transitions"],
                     "attempts": [], "epoch": 0, "max_attempts": MAX_ATTEMPTS}
            state_file = job / "adaptive-state.json"
            trace_pcs, trace_edges, trace_cases = set(), set(), []
            deviations_found = []
            stop = "limite"
            for _ in range(MAX_ATTEMPTS):
                private_write(state_file, state)
                result = subprocess.run([str(ENGINE), "--wcop-adapt-plan",
                                         str(job / "engine-impl.json"), str(state_file)],
                                        capture_output=True, text=True, timeout=15)
                if result.returncode:
                    raise RuntimeError("planejador Rust: " + result.stderr[:180])
                plan = json.loads(result.stdout)
                if plan.get("status") != "next":
                    stop = plan.get("status", "interrompido")
                    break
                item = plan["candidate"]
                trace = rpc(url, "debug_traceCall", [{"from": item["sender"], "to": TOKEN,
                          "data": item["data"]}, "latest",
                          {"disableStorage": True, "disableMemory": True, "disableStack": True}])
                logs = trace.get("structLogs") or []
                pcs = {int(row["pc"]) for row in logs if row.get("depth") == 2}
                known = pcs & instructions
                blocks = [block_for.get(int(row["pc"])) for row in logs if row.get("depth") == 2]
                edges = {(a, b) for a, b in zip(blocks, blocks[1:]) if a and b and a != b}
                previous_pcs = set(state["visited_pcs"])
                previous_edges = {tuple(e) for e in state["visited_edges"]}
                new_pcs = known - previous_pcs
                new_edges = edges - previous_edges
                state["visited_pcs"] = sorted(previous_pcs | known)
                state["visited_edges"] = [list(e) for e in sorted(previous_edges | edges)]
                trace_pcs.update(known)
                trace_edges.update(edges)
                failed = bool(trace.get("failed"))
                committed = False
                discrepancy = []
                repeatable = False
                before = after = None
                if not failed:
                    try:
                        before = view(url, item)
                        snapshot = rpc(url, "evm_snapshot", [])
                        tx = rpc(url, "eth_sendTransaction", [{"from": item["sender"], "to": TOKEN,
                                  "data": item["data"], "gas": "0x989680"}])
                        receipt = wait_receipt(url, tx)
                        committed = bool(receipt and receipt.get("status") == "0x1")
                        if committed:
                            after = view(url, item)
                            discrepancy = deviations(item, before, after)
                            if discrepancy:
                                rpc(url, "evm_revert", [snapshot])
                                replay_before = view(url, item)
                                tx2 = rpc(url, "eth_sendTransaction", [{"from": item["sender"], "to": TOKEN,
                                           "data": item["data"], "gas": "0x989680"}])
                                receipt2 = wait_receipt(url, tx2)
                                replay_after = view(url, item)
                                repeatable = bool(receipt2 and receipt2.get("status") == "0x1"
                                                  and deviations(item, replay_before, replay_after) == discrepancy)
                                deviations_found.append({"candidate_id": item["id"],
                                                         "checks": discrepancy, "repeatable_locally": repeatable})
                    except (OSError, ValueError, KeyError, TypeError, TimeoutError) as error:
                        discrepancy = ["local_execution_inconclusive:" + type(error).__name__]
                row = {"id": item["id"], "family": item["family"], "selector": item["selector"],
                       "epoch": state["epoch"], "sender": item["sender"], "amount": item["amount"],
                       "failed": failed, "committed_locally": committed, "new_pcs": len(new_pcs),
                       "new_edges": len(new_edges),
                       "new_uncertain_pcs": len(new_pcs & uncertain),
                       "deviation": discrepancy, "repeatable_deviation": repeatable,
                       "score": plan["score"]}
                state["attempts"].append(row)
                grow_candidates(state, item, new_pcs, new_edges, discrepancy)
                trace_cases.append({"case": item["id"], "group": "adaptativo", "selector": item["selector"],
                                    "failed": failed, "steps": len(logs), "implementation_steps": len(pcs),
                                    "known_pcs": len(known), "unknown_pcs": len(pcs - instructions),
                                    "state_committed_locally": committed})
            private_write(state_file, state)
            trace_output = {"measurement": "ciclo local orientado por resultados",
                            "block": block, "contract": TOKEN, "implementation": implementation,
                            "engine_code_sha256": code_hash, "cases": trace_cases,
                            "pcs": sorted(trace_pcs), "observed_instruction_pcs": len(trace_pcs),
                            "observed_transitions": [list(edge) for edge in sorted(trace_edges)]}
            private_write(job / "adaptive-trace.json", trace_output)
            trace_hash = digest((job / "adaptive-trace.json").read_bytes())
            summary = {"format": 1, "contract": TOKEN, "block": block,
                       "engine_code_sha256": code_hash, "status": stop,
                       "candidate_count": len(pool), "attempts": len(state["attempts"]),
                       "baseline_pcs": len(set(baseline["pcs"])),
                       "combined_pcs": len(state["visited_pcs"]),
                       "new_pcs": len(set(state["visited_pcs"]) - set(baseline["pcs"])),
                       "new_edges": len(set(map(tuple, state["visited_edges"]))
                                        - set(map(tuple, baseline["observed_transitions"]))),
                       "deviations": deviations_found,
                       "trace_sha256": trace_hash,
                       "limitation": "ciclo limitado a entradas geradas e a uma cópia local do bloco; desvios exigem revisão"}
            private_write(job / "adaptive.json", summary)
            return summary
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    result = run(sys.argv[1])
    print(json.dumps({k: result[k] for k in ("status", "attempts", "new_pcs", "new_edges", "deviations")},
                     ensure_ascii=False))
