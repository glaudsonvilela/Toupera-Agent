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
MAX_ATTEMPTS = 40


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
    if kind in ("transfer", "transferFrom", "mint", "burn", "burnFrom"):
        result["owner_balance"] = balance(url, owner)
        if recipient and recipient.lower() != owner.lower():
            result["recipient_balance"] = balance(url, recipient)
    if kind in ("transferFrom", "burnFrom"):
        result["allowance"] = uint(url, "0xdd62ed3e" + word_address(owner) + word_address(sender))
    if kind == "permit":
        result["allowance"] = uint(url, "0xdd62ed3e" + word_address(owner) + word_address(recipient))
        result["nonce"] = uint(url, "0x7ecebe00" + word_address(owner))
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
    expected_supply = amount if kind == "mint" else -amount if kind in ("burn", "burnFrom") else 0
    if after["supply"] - before["supply"] != expected_supply:
        errors.append("supply_delta")
    if kind in ("transfer", "transferFrom", "mint", "burn", "burnFrom"):
        expected_owner = (-amount if kind in ("transfer", "transferFrom", "burn", "burnFrom") else 0)
        if after["owner_balance"] - before["owner_balance"] != expected_owner:
            errors.append("owner_balance_delta")
        if kind not in ("burn", "burnFrom") and item.get("recipient", "").lower() != item.get("owner", item["sender"]).lower():
            if after.get("recipient_balance", 0) - before.get("recipient_balance", 0) != amount:
                errors.append("recipient_balance_delta")
    if kind in ("transferFrom", "burnFrom"):
        initial = before.get("allowance")
        expected = initial if initial == 2**256 - 1 else max(0, initial - amount)
        if after.get("allowance") != expected:
            errors.append("allowance_delta")
    if kind == "permit":
        if after.get("allowance") != amount: errors.append("permit_allowance")
        if after.get("nonce") != before.get("nonce", 0) + 1: errors.append("permit_nonce")
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
    def add(family, selector, data, sender, amount=0, recipient=None, owner=None, role=None, setup=None):
        if not sender:
            return
        base = {"family": family, "selector": selector, "data": data, "sender": sender,
                "amount": str(amount), "boundary": amount in (0, min(holder_balance + 1, 10**24 + 1))}
        if recipient: base["recipient"] = recipient
        if owner: base["owner"] = owner
        if role: base["role_hash"] = role
        if setup: base["setup"] = setup
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
            add("burnFrom", "0x79cc6790", "0x79cc6790" + word_address(holder)
                + word_number(amount), SECOND, amount, None, holder)
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
    if holder:
        max_uint = 2**256 - 1
        add("approve", "0x095ea7b3", "0x095ea7b3" + word_address(SECOND)
            + word_number(max_uint), holder, max_uint, SECOND)
        unlimited = [{"sender": holder, "data": "0x095ea7b3" + word_address(SECOND)
                      + word_number(max_uint)}]
        for family, prefix, tail, recipient in (
            ("transferFrom", "0x23b872dd", word_address(holder) + word_address(RECEIVER), RECEIVER),
            ("burnFrom", "0x79cc6790", word_address(holder), None)):
            add(family, prefix, prefix + tail + word_number(1), SECOND, 1,
                recipient, holder, setup=unlimited)
        for v, r, signature_s in ((0, 0, 0), (27, 0, 0), (28, 1, 1), (27, 1, 1)):
            permit = ("0xd505accf" + word_address(holder) + word_address(SECOND)
                      + word_number(1) + word_number(max_uint) + word_number(v)
                      + word_number(r) + word_number(signature_s))
            add("permit", "0xd505accf", permit, DEFAULT, 1, SECOND, holder)
        for amount in (1, min(holder_balance, 11)):
            approval = [{"sender": holder, "data": "0x095ea7b3" + word_address(SECOND)
                         + word_number(amount)}]
            add("transferFrom", "0x23b872dd", "0x23b872dd" + word_address(holder)
                + word_address(RECEIVER) + word_number(amount), SECOND, amount, RECEIVER, holder,
                setup=approval)
            add("burnFrom", "0x79cc6790", "0x79cc6790" + word_address(holder)
                + word_number(amount), SECOND, amount, None, holder, setup=approval)
        if minter:
            for amount in (1, 11):
                funding = [{"sender": minter, "data": "0x40c10f19" + word_address(SECOND)
                            + word_number(amount)}]
                add("burn", "0x42966c68", "0x42966c68" + word_number(amount),
                    SECOND, amount, None, SECOND, setup=funding)
                add("transfer", "0xa9059cbb", "0xa9059cbb" + word_address(RECEIVER)
                    + word_number(amount), SECOND, amount, RECEIVER, SECOND, setup=funding)
        if pauser:
            paused = [{"sender": pauser, "data": "0x8456cb59"}]
            add("transfer", "0xa9059cbb", "0xa9059cbb" + word_address(SECOND)
                + word_number(1), holder, 1, SECOND, holder, setup=paused)
    if admin and role_hash:
        grant = [{"sender": admin, "data": "0x2f2ff15d" + role_hash.removeprefix("0x")
                  + word_address(SECOND)}]
        add("mint", "0x40c10f19", "0x40c10f19" + word_address(RECEIVER)
            + word_number(11), SECOND, 11, RECEIVER, SECOND, setup=grant)
    if minter and holder:
        funding = {"sender": minter, "data": "0x40c10f19" + word_address(SECOND) + word_number(1)}
        approval = {"sender": SECOND, "data": "0x095ea7b3" + word_address(RECEIVER) + word_number(1)}
        add("transferFrom", "0x23b872dd", "0x23b872dd" + word_address(SECOND)
            + word_address(holder) + word_number(1), RECEIVER, 1, holder, SECOND,
            setup=[funding, approval])
    if pauser and holder:
        approval = {"sender": holder, "data": "0x095ea7b3" + word_address(SECOND) + word_number(1)}
        paused = {"sender": pauser, "data": "0x8456cb59"}
        add("transferFrom", "0x23b872dd", "0x23b872dd" + word_address(holder)
            + word_address(RECEIVER) + word_number(1), SECOND, 1, RECEIVER, holder,
            setup=[approval, paused])
    return list({item["id"]: item for item in out}.values())[:256]


def related_values(item):
    amount = int(item.get("amount", 0))
    if item["family"] not in ("transfer", "transferFrom", "mint", "burn", "burnFrom", "approve"):
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
    elif family == "burnFrom":
        data = prefix + word_address(owner) + word_number(amount)
    elif family in ("mint", "approve"):
        data = prefix + word_address(recipient) + word_number(amount)
    elif family == "burn":
        data = prefix + word_number(amount)
    else:
        return None
    result = dict(item, data=data, amount=str(amount), boundary=True, derived_from=item["id"])
    if item.get("setup"):
        setup = [dict(step) for step in item["setup"]]
        for step in setup:
            if step["data"].startswith("0x095ea7b3") and int(step["data"][-64:], 16) != 2**256 - 1:
                step["data"] = step["data"][:-64] + word_number(amount)
            elif step["data"].startswith("0x40c10f19"):
                step["data"] = step["data"][:-64] + word_number(amount)
        result["setup"] = setup
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



def evaluate_item(url, item, instructions, block_for):
    setup = item.get("setup") or []
    outer_snapshot = rpc(url, "evm_snapshot", []) if setup else None
    setup_ok = True
    try:
        for operation in setup:
            tx = rpc(url, "eth_sendTransaction", [{"from": operation["sender"], "to": TOKEN,
                      "data": operation["data"], "gas": "0x989680"}])
            if wait_receipt(url, tx).get("status") != "0x1":
                setup_ok = False
                break
        if setup_ok:
            trace = rpc(url, "debug_traceCall", [{"from": item["sender"], "to": TOKEN,
                        "data": item["data"]}, "latest",
                        {"disableStorage": True, "disableMemory": True, "disableStack": True}])
        else:
            trace = {"failed": True, "structLogs": []}
        logs = trace.get("structLogs") or []
        pcs = {int(row["pc"]) for row in logs if row.get("depth") == 2}
        known = pcs & instructions
        blocks = [block_for.get(int(row["pc"])) for row in logs if row.get("depth") == 2]
        edges = {(a, b) for a, b in zip(blocks, blocks[1:]) if a and b and a != b}
        failed = bool(trace.get("failed"))
        committed = False
        discrepancy = []
        repeatable = False
        if not failed:
            try:
                before = view(url, item)
                snapshot = rpc(url, "evm_snapshot", [])
                tx = rpc(url, "eth_sendTransaction", [{"from": item["sender"], "to": TOKEN,
                          "data": item["data"], "gas": "0x989680"}])
                receipt = wait_receipt(url, tx)
                committed = receipt.get("status") == "0x1"
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
                        repeatable = (receipt2.get("status") == "0x1"
                                      and deviations(item, replay_before, replay_after) == discrepancy)
            except (OSError, ValueError, KeyError, TypeError, TimeoutError) as error:
                discrepancy = ["local_execution_inconclusive:" + type(error).__name__]
        return {"logs": logs, "pcs": pcs, "known": known, "blocks": blocks,
                "edges": edges, "failed": failed, "committed": committed,
                "discrepancy": discrepancy, "repeatable": repeatable,
                "setup_ok": setup_ok, "setup_steps": len(setup)}
    finally:
        if outer_snapshot is not None:
            rpc(url, "evm_revert", [outer_snapshot])

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
            focus = read(job / "focus.json") if (job / "focus.json").is_file() else {}
            if focus and (focus.get("contract") != TOKEN or focus.get("block") != block
                          or focus.get("engine_code_sha256") != code_hash
                          or focus.get("observed_pcs") != len(set(baseline["pcs"]))):
                raise ValueError("mapa inicial fora do bloco")
            state = {"format": 1, "address": TOKEN, "block": block,
                     "focus_engine_code_sha256": code_hash if focus else None,
                     "focus_by_selector": focus.get("selector_focus") or {},
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
                measured = evaluate_item(url, item, instructions, block_for)
                logs, pcs, known, edges = (measured[key] for key in ("logs", "pcs", "known", "edges"))
                previous_pcs = set(state["visited_pcs"])
                previous_edges = {tuple(edge) for edge in state["visited_edges"]}
                new_pcs = known - previous_pcs
                new_edges = edges - previous_edges
                prior_blocks = {block_for[pc] for pc in previous_pcs if pc in block_for}
                new_blocks = {block_for[pc] for pc in new_pcs if pc in block_for} - prior_blocks
                frontier_crossings = {(left, right) for left, right in edges
                                      if left in prior_blocks and right in new_blocks}
                state["visited_pcs"] = sorted(previous_pcs | known)
                state["visited_edges"] = [list(edge) for edge in sorted(previous_edges | edges)]
                trace_pcs.update(known)
                trace_edges.update(edges)
                discrepancy = measured["discrepancy"]
                if discrepancy:
                    deviations_found.append({"candidate_id": item["id"], "checks": discrepancy,
                                             "repeatable_locally": measured["repeatable"]})
                row = {"id": item["id"], "family": item["family"], "selector": item["selector"],
                       "epoch": state["epoch"], "sender": item["sender"], "amount": item["amount"],
                       "failed": measured["failed"], "committed_locally": measured["committed"],
                       "setup_steps": measured["setup_steps"], "setup_ok": measured["setup_ok"],
                       "blocks": sorted(set(block for block in measured["blocks"] if block)),
                       "new_pcs": len(new_pcs), "new_edges": len(new_edges),
                       "new_blocks": len(new_blocks),
                       "frontier_crossings": len(frontier_crossings),
                       "frontier_blocks_total": plan.get("frontier_blocks_total", 0),
                       "reachable_frontiers": plan.get("reachable_frontiers", 0),
                       "new_uncertain_pcs": len(new_pcs & uncertain),
                       "deviation": discrepancy, "repeatable_deviation": measured["repeatable"],
                       "score": plan["score"]}
                state["attempts"].append(row)
                grow_candidates(state, item, new_pcs, new_edges, discrepancy)
                trace_cases.append({"case": item["id"], "group": "adaptativo",
                                    "selector": item["selector"], "failed": measured["failed"],
                                    "steps": len(logs), "implementation_steps": len(pcs),
                                    "known_pcs": len(known), "unknown_pcs": len(pcs - instructions),
                                    "setup_steps": measured["setup_steps"],
                                    "setup_ok": measured["setup_ok"],
                                    "state_committed_locally": measured["committed"]})
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
                       "setup_attempts": sum(bool(row["setup_steps"]) for row in state["attempts"]),
                       "frontier_crossings": sum(row["frontier_crossings"] for row in state["attempts"]),
                       "new_blocks": len({block_for[pc] for pc in state["visited_pcs"] if pc in block_for}
                                         - {block_for[pc] for pc in baseline["pcs"] if pc in block_for}),
                       "local_commits": sum(bool(row["committed_locally"]) for row in state["attempts"]),
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
