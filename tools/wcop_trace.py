#!/usr/bin/env python3
"""Observe implementation program counters in a bounded local fork of one pinned wCOP block."""
import json
import hashlib
import os
from pathlib import Path
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output/site-jobs"
ANVIL = "/home/toupera/.foundry/bin/anvil"
UPSTREAM = "https://ethereum.publicnode.com"
TOKEN = "0x8a1d45e102e886510e891d2ec656a708991e2d76"
DEFAULT = "0x000000000000000000000000000000000000beef"
RECEIVER = "0x000000000000000000000000000000000000cafe"
SECOND = "0x000000000000000000000000000000000000d00d"
SLOT = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def private_write(path, data):
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    tmp.chmod(0o600)
    tmp.replace(path)


def rpc(url, method, params):
    payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, payload, {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=20) as response:
        answer = json.load(response)
    if "error" in answer:
        raise ValueError(str(answer["error"])[:160])
    return answer["result"]


def word_address(value):
    if not isinstance(value, str) or len(value) != 42 or not value.startswith("0x"):
        raise ValueError("endereço inválido")
    int(value[2:], 16)
    return value[2:].lower().rjust(64, "0")


def word_number(value):
    if not isinstance(value, int) or value < 0 or value >= 2**256:
        raise ValueError("número inválido")
    return format(value, "064x")


def available_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def trace_job(job_id, capture_stack=False, on_trace=None, write_summary=True):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    history = read(job / "history.json")
    scope = read(job / "scope.json")
    roles = read(job / "roles.json")
    engine = read(job / "engine-impl.json")
    status = read(job / "status.json")
    if status.get("state") not in ("complete", "running") or history.get("address") != TOKEN:
        raise ValueError("trabalho completo do contrato esperado necessário")
    block = int(history["through_block"])
    active = roles.get("active_accounts_from_events") or {}
    actor = {name: (accounts[0] if accounts else None) for name, accounts in active.items()}
    holder = (history.get("funded_holder") or {}).get("address")
    selectors = (scope.get("inventory") or {}).get("functions") or []
    cases = []
    for item in selectors:
        if item.get("signature", "").endswith("()"):
            cases.append((item["signature"], item["selector"], DEFAULT))
    if holder:
        cases.extend([
            ("balanceOf(holder)", "0x70a08231" + word_address(holder), DEFAULT),
            ("transfer(holder)", "0xa9059cbb" + word_address(RECEIVER) + word_number(1), holder),
            ("burn(holder)", "0x42966c68" + word_number(1), holder),
            ("approve(holder)", "0x095ea7b3" + word_address(RECEIVER) + word_number(1), holder),
        ])
    minter = actor.get("MINTER_ROLE")
    pauser = actor.get("PAUSER_ROLE")
    if minter:
        data = "0x40c10f19" + word_address(RECEIVER) + word_number(1)
        cases.extend([("mint(authorized)", data, minter), ("mint(other)", data, DEFAULT)])
    if pauser:
        cases.extend([("pause(authorized)", "0x8456cb59", pauser),
                      ("pause(other)", "0x8456cb59", DEFAULT)])
    if actor.get("DEFAULT_ADMIN_ROLE") and minter:
        role_hash = roles["role_hashes"]["MINTER_ROLE"].removeprefix("0x")
        cases.append(("grantRole(admin)", "0x2f2ff15d" + role_hash + word_address(RECEIVER), actor["DEFAULT_ADMIN_ROLE"]))
    instructions = {int(item["pc"]) for item in engine.get("instructions") or []}
    block_for = {int(pc): item["id"] for item in engine.get("blocks") or [] for pc in item.get("instruction_pcs") or []}
    static_edges = {(item["from"], item["to"]) for item in engine.get("control_edges") or []}
    port = available_port()
    url = "http://127.0.0.1:" + str(port)
    log_path = job / "tools/anvil-trace.log"
    log_path.parent.mkdir(exist_ok=True, mode=0o700)
    visited = set()
    visited_blocks = set()
    observed_edges = set()
    results = []
    with log_path.open("w", encoding="utf-8") as output:
        process = subprocess.Popen([ANVIL, "--fork-url", UPSTREAM, "--fork-block-number", str(block),
                                    "--host", "127.0.0.1", "--port", str(port), "--silent"],
                                   stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT)
        try:
            for attempt in range(20):
                if process.poll() is not None:
                    raise RuntimeError("cópia local terminou antes de iniciar")
                try:
                    if int(rpc(url, "eth_chainId", []), 16) == 1:
                        break
                except (OSError, ValueError):
                    time.sleep(0.5)
            else:
                raise RuntimeError("cópia local não respondeu")
            implementation_word = rpc(url, "eth_getStorageAt", [TOKEN, SLOT, "latest"])
            implementation_address = "0x" + implementation_word[-40:].lower()
            code = rpc(url, "eth_getCode", [implementation_address, "latest"])
            recorded_code = (job / "implementation.hex").read_text(encoding="utf-8").strip()
            if code.lower() != recorded_code.lower():
                raise ValueError("implementação da cópia local diverge do bloco fixado")
            def observe(name, data, sender, group, expected_failure=None):
                try:
                    trace = rpc(url, "debug_traceCall", [{"from": sender, "to": TOKEN, "data": data},
                                                         "latest", {"disableStorage": True, "disableMemory": True,
                                                                    "disableStack": not capture_stack}])
                    logs = trace.get("structLogs") or []
                    pcs = [int(item["pc"]) for item in logs if item.get("depth") == 2]
                    known = {pc for pc in pcs if pc in instructions}
                    visited.update(known)
                    blocks = [block_for.get(pc) for pc in pcs]
                    visited_blocks.update(item for item in blocks if item)
                    observed_edges.update((a, b) for a, b in zip(blocks, blocks[1:])
                                          if a and b and a != b)
                    failed = bool(trace.get("failed"))
                    row = {"case": name, "group": group, "selector": data[:10], "failed": failed,
                           "steps": len(logs), "implementation_steps": len(pcs),
                           "known_pcs": len(known), "unknown_pcs": len(set(pcs) - instructions)}
                    if expected_failure is not None:
                        row["expected_outcome_matches"] = failed == expected_failure
                    results.append(row)
                    if on_trace is not None:
                        on_trace(name, group, data[:10], logs, failed, row)
                    return failed
                except (OSError, ValueError, KeyError, TypeError) as error:
                    results.append({"case": name, "group": group, "selector": data[:10],
                                    "error": type(error).__name__ + ": " + str(error)[:120]})
                    return None
            for name, data, sender in cases:
                observe(name, data, sender, "isolada")
            admin = actor.get("DEFAULT_ADMIN_ROLE")
            if minter and pauser and admin:
                try:
                    for account in {minter, pauser, admin, RECEIVER, SECOND}:
                        rpc(url, "anvil_impersonateAccount", [account])
                        rpc(url, "anvil_setBalance", [account, hex(10**18)])
                    role_hash = roles["role_hashes"]["MINTER_ROLE"].removeprefix("0x")
                    transfer = "0xa9059cbb" + word_address(RECEIVER) + word_number(1)
                    mint_one = "0x40c10f19" + word_address(RECEIVER) + word_number(1)
                    sequence = [
                        ("sequência: emitir", "0x40c10f19" + word_address(RECEIVER) + word_number(11), minter, True, False),
                        ("sequência: autorizar", "0x095ea7b3" + word_address(SECOND) + word_number(5), RECEIVER, True, False),
                        ("sequência: mover com autorização", "0x23b872dd" + word_address(RECEIVER) + word_address(SECOND) + word_number(5), SECOND, True, False),
                        ("sequência: retirar", "0x42966c68" + word_number(2), SECOND, True, False),
                        ("sequência: pausar", "0x8456cb59", pauser, True, False),
                        ("sequência: movimento pausado", transfer, SECOND, False, True),
                        ("sequência: retomar", "0x3f4ba83a", pauser, True, False),
                        ("sequência: movimento retomado", transfer, SECOND, True, False),
                        ("sequência: conceder papel", "0x2f2ff15d" + role_hash + word_address(SECOND), admin, True, False),
                        ("sequência: emitir com papel", mint_one, SECOND, True, False),
                        ("sequência: retirar papel", "0xd547741f" + role_hash + word_address(SECOND), admin, True, False),
                        ("sequência: emitir após retirada", mint_one, SECOND, False, True),
                    ]
                    for name, data, sender, commit, expected_failure in sequence:
                        failed = observe(name, data, sender, "sequência", expected_failure)
                        if failed is None or failed != expected_failure:
                            raise ValueError("resultado local diferente do esperado em " + name)
                        if commit:
                            tx_hash = rpc(url, "eth_sendTransaction", [{"from": sender, "to": TOKEN,
                                                                        "data": data, "gas": "0x989680"}])
                            receipt = None
                            for _ in range(20):
                                receipt = rpc(url, "eth_getTransactionReceipt", [tx_hash])
                                if receipt is not None:
                                    break
                                time.sleep(0.2)
                            if receipt is None or receipt.get("status") != "0x1":
                                raise ValueError("operação local não concluída em " + name)
                            results[-1]["state_committed_locally"] = True
                except (OSError, ValueError, KeyError, TypeError) as error:
                    results.append({"case": "sequência interrompida", "group": "sequência",
                                    "error": type(error).__name__ + ": " + str(error)[:120]})
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    summary = {"measurement": "debug_traceCall em Anvil local no bloco fixado; chamadas isoladas e sequência local com estado",
               "block": block, "contract": TOKEN, "implementation": implementation_address,
               "engine_code_sha256": hashlib.sha256(bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))).hexdigest(),
               "implementation_instructions": len(instructions),
               "observed_instruction_pcs": len(visited), "observed_blocks": len(visited_blocks),
               "engine_blocks": len(engine.get("blocks") or []), "observed_block_transitions": len(observed_edges),
               "static_edges_matched": len(observed_edges & static_edges), "engine_static_edges": len(static_edges),
               "cases": results, "pcs": sorted(visited),
               "observed_transitions": [list(edge) for edge in sorted(observed_edges)],
               "unmatched_transitions": len(observed_edges - static_edges),
               "limitation": "Não mede todos os estados, nem prova que os PCs restantes sejam alcançáveis."}
    if write_summary:
        private_write(job / "trace.json", summary)
    return summary


if __name__ == "__main__":
    result = trace_job(sys.argv[1])
    print(json.dumps({k: result[k] for k in ("block", "observed_instruction_pcs", "implementation_instructions",
                                             "observed_blocks", "observed_block_transitions", "static_edges_matched",
                                             "unmatched_transitions")}, ensure_ascii=False))
