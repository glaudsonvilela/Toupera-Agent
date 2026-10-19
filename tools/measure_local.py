#!/usr/bin/env python3
"""Measure two fixed input sets on owned fixture contracts in a child Anvil.

Runs only on 127.0.0.1. This is a calibration harness, not a production claim.
"""
import json
import pathlib
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOLC = ROOT / "private-output/real-targets/yield-vault/solc-0.8.24"
ANVIL = pathlib.Path("/home/toupera/.foundry/bin/anvil")
FILES = [
    ("RoundingVaultM7Fixture", "RoundingVaultM7Fixture.sol", "withdraw(uint256)", "previewWithdraw(uint256)", True),
    ("ModifierFlowFixture", "ModifierFlowFixture.sol", "deposit(uint256)", None, False),
    ("VaultLoopFixture", "VaultLoopFixture.sol", "sumUntil(uint256)", None, False),
]
BASELINE = list(range(1, 9))


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def rpc(url, method, params):
    if not url.startswith("http://127.0.0.1:"):
        raise RuntimeError("local RPC only")
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as response:
        value = json.load(response)
    if "error" in value:
        raise RuntimeError(f"{method}: {value['error'].get('message', 'RPC error')}")
    return value["result"]


def compile_fixture(filename, contract):
    source = (ROOT / "tests/fixtures" / filename).read_text()
    request = {
        "language": "Solidity", "sources": {filename: {"content": source}},
        "settings": {"outputSelection": {"*": {"": ["ast"], "*": ["evm.bytecode.object", "evm.methodIdentifiers"]}}},
    }
    process = subprocess.run([str(SOLC), "--standard-json"], input=json.dumps(request), text=True,
                             capture_output=True, timeout=30, check=True)
    raw = process.stdout
    output = json.loads(raw[raw.find("{"):])
    errors = [e for e in output.get("errors", []) if e.get("severity") == "error"]
    if errors:
        raise RuntimeError(f"solc rejected {filename}")
    return output["contracts"][filename][contract]["evm"], output["sources"][filename]["ast"]


def nodes(value):
    if isinstance(value, dict):
        if "nodeType" in value:
            yield value
        for child in value.values():
            yield from nodes(child)
    elif isinstance(value, list):
        for child in value:
            yield from nodes(child)


def selected_inputs(ast, method):
    """Small values plus exact direct first-parameter/literal boundaries."""
    selected = {0, 1}
    function = next((node for node in nodes(ast)
                     if node.get("nodeType") == "FunctionDefinition" and node.get("name") == method), None)
    if function is None:
        return sorted(selected)
    params = function.get("parameters", {}).get("parameters", [])
    first_id = params[0].get("id") if params else None
    if first_id is None:
        return sorted(selected)
    for node in nodes(function.get("body", {})):
        if node.get("nodeType") != "BinaryOperation":
            continue
        left, right = node.get("leftExpression", {}), node.get("rightExpression", {})
        if left.get("nodeType") == "Identifier" and left.get("referencedDeclaration") == first_id:
            literal = right
        elif right.get("nodeType") == "Identifier" and right.get("referencedDeclaration") == first_id:
            literal = left
        else:
            continue
        if literal.get("kind") != "number":
            continue
        try:
            value = int(literal["value"].replace("_", ""))
        except (ValueError, KeyError):
            continue
        if value >= 0:
            selected.add(value)
            if node.get("operator") in ("<", ">="):
                selected.add(max(value - 1, 0))
            else:
                selected.add(value + 1)
    return sorted(selected)[:8]


def wait_receipt(url, tx_hash):
    for _ in range(100):
        receipt = rpc(url, "eth_getTransactionReceipt", [tx_hash])
        if receipt is not None:
            return receipt
        time.sleep(0.05)
    raise RuntimeError("local receipt timeout")


def deploy(url, actor, creation_hex):
    tx_hash = rpc(url, "eth_sendTransaction", [{"from": actor, "data": "0x" + creation_hex, "gas": "0x3d0900"}])
    receipt = wait_receipt(url, tx_hash)
    if receipt["status"] != "0x1":
        raise RuntimeError("fixture deployment failed")
    return receipt["contractAddress"]


def branch_outcomes(trace):
    outcomes = set()
    for step in trace.get("structLogs", []):
        if step.get("op") == "JUMPI":
            stack = step.get("stack", [])
            if len(stack) >= 2:
                outcomes.add((step["pc"], "taken" if int(stack[-2], 16) != 0 else "not_taken"))
    return outcomes


def run_set(url, actor, address, selector, preview_selector, amounts):
    outcomes = set()
    completed = 0
    numeric_mismatch = []
    rpc_calls = 0
    start = time.perf_counter()

    def call(method, params):
        nonlocal rpc_calls
        rpc_calls += 1
        return rpc(url, method, params)

    for amount in amounts:
        snap = call("evm_snapshot", [])
        data = "0x" + selector + f"{amount:064x}"
        if preview_selector:
            preview = call("eth_call", [{"from": actor, "to": address, "data": "0x" + preview_selector + f"{amount:064x}"}, "latest"])
            try:
                actual = call("eth_call", [{"from": actor, "to": address, "data": data}, "latest"])
                if int(preview, 16) != int(actual, 16):
                    numeric_mismatch.append(amount)
            except RuntimeError:
                pass
        try:
            tx = call("eth_sendTransaction", [{"from": actor, "to": address, "data": data, "gas": "0x2dc6c0"}])
            receipt = call("eth_getTransactionReceipt", [tx])
            if receipt is None:
                receipt = wait_receipt(url, tx)
                rpc_calls += 1
            completed += receipt["status"] == "0x1"
            trace = call("debug_traceTransaction", [tx, {"disableMemory": True, "disableStack": False, "disableStorage": True}])
            outcomes.update(branch_outcomes(trace))
        finally:
            if call("evm_revert", [snap]) is not True:
                raise RuntimeError("local snapshot restore failed")
    return {"inputs": amounts, "cases": len(amounts), "completed": completed,
            "root_outcomes": len(outcomes), "outcome_keys": sorted(f"{pc}:{way}" for pc, way in outcomes),
            "rpc_calls": rpc_calls, "numeric_mismatch_inputs": numeric_mismatch,
            "elapsed_ms": round((time.perf_counter() - start) * 1000, 2)}


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: measure_local.py <private-output.json>")
    output_path = pathlib.Path(sys.argv[1])
    if not output_path.is_absolute() or "private-output" not in output_path.parts:
        raise SystemExit("output must be an absolute private-output path")
    p = port()
    url = f"http://127.0.0.1:{p}"
    child = subprocess.Popen([str(ANVIL), "--host", "127.0.0.1", "--port", str(p), "--steps-tracing"],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            if child.poll() is not None:
                raise RuntimeError("Anvil exited")
            try:
                actor = rpc(url, "eth_accounts", [])[0]
                break
            except Exception:
                time.sleep(0.05)
        else:
            raise RuntimeError("Anvil did not start")
        rows = []
        for contract, filename, signature, preview_signature, faulty in FILES:
            compiled, ast = compile_fixture(filename, contract)
            code = compiled["bytecode"]["object"]
            if faulty:
                code += f"{1:064x}"
            address = deploy(url, actor, code)
            methods = compiled["methodIdentifiers"]
            amounts_selected = selected_inputs(ast, signature.split("(")[0])
            row = {"fixture": contract, "method": signature, "environment": "child_anvil_loopback",
                   "selected_basis": "zero_one_plus_direct_first_parameter_literal_boundaries",
                   "selected_inputs": amounts_selected}
            for label, amounts in {"sequential_1_to_8": BASELINE, "selected": amounts_selected}.items():
                row[label] = run_set(url, actor, address, methods[signature],
                                     methods[preview_signature] if preview_signature else None, amounts)
            rows.append(row)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps({"schema": "local-fixture-measure-v1", "rows": rows}, indent=2) + "\n")
    finally:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)


if __name__ == "__main__":
    main()
