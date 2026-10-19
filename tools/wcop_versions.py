#!/usr/bin/env python3
"""Collect a bounded timeline of published versions at fixed blocks."""
import hashlib
import json
from pathlib import Path
import sys
import urllib.error
from site_jobs import rpc, SLOT, ADDRESS, WORK, CHAIN, jwrite

def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))

def fixed_call(block, data):
    return rpc("eth_call", [{"to": ADDRESS, "data": data}, hex(block)])

def snapshot(block, roles, accounts):
    header = rpc("eth_getBlockByNumber", [hex(block), False])
    if not header or int(header["number"], 16) != block:
        raise ValueError("bloco indisponível")
    try:
        slot = rpc("eth_getStorageAt", [ADDRESS, SLOT, hex(block)])
        implementation = "0x" + slot[-40:].lower()
        code = rpc("eth_getCode", [implementation, hex(block)])
    except (urllib.error.HTTPError, urllib.error.URLError, ValueError, OSError):
        return {"block": block, "block_hash": header["hash"], "available": False,
                "reason": "estado histórico indisponível neste provedor"}
    result = {"block": block, "block_hash": header["hash"], "available": True,
              "implementation": implementation,
              "code_sha256": hashlib.sha256(bytes.fromhex(code.removeprefix("0x"))).hexdigest(),
              "code_bytes": len(code.removeprefix("0x")) // 2,
              "total_supply_raw": None, "roles_seen_active": {}, "role_reads_incomplete": []}
    try:
        result["total_supply_raw"] = str(int(fixed_call(block, "0x18160ddd"), 16))
    except (ValueError, OSError, KeyError):
        pass
    for name, role in (roles.get("role_hashes") or {}).items():
        active = 0
        for account in accounts:
            data = "0x91d14854" + role.removeprefix("0x") + account.removeprefix("0x").rjust(64, "0")
            try:
                active += int(fixed_call(block, data), 16) != 0
            except (ValueError, OSError, KeyError):
                result["role_reads_incomplete"].append(name + ":" + account)
        result["roles_seen_active"][name] = active
    return result

def main(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    history = read(job / "history.json")
    roles = read(job / "roles.json")
    if history["address"] != ADDRESS or history["complete_event_range"] is not True:
        raise ValueError("histórico incompleto")
    if int(rpc("eth_chainId", []), 16) != CHAIN:
        raise ValueError("rede diferente")
    upgrades = sorted((e for e in history["timeline"] if e["kind"] == "upgraded"),
                      key=lambda x: (x["block"], x["log_index"]))
    if len(upgrades) > 16:
        raise ValueError("muitas mudanças para esta leitura")
    block = int(history["through_block"])
    points = {block}
    for e in upgrades:
        n = int(e["block"])
        if not history["creation_block"] <= n <= block:
            raise ValueError("registro fora do intervalo")
        points.add(n)
        if n > history["creation_block"]:
            points.add(n - 1)
    if len(points) > 34:
        raise ValueError("limite de blocos")
    accounts = sorted({e["account"] for e in history["timeline"]
                       if e["kind"] in ("role_granted", "role_revoked") and "account" in e})
    if len(accounts) > 64:
        raise ValueError("muitas contas")
    rows = [snapshot(n, roles, accounts) for n in sorted(points)]
    current = (job / "implementation.hex").read_text().strip()
    if not rows[-1].get("available") and (job / "decision.json").is_file():
        decision = read(job / "decision.json")
        manifest = read(job / "job-manifest.json")
        if (decision.get("identity_ok") is True and decision.get("block_number") == block
                and manifest.get("block_hash") == rows[-1]["block_hash"]
                and manifest.get("implementation") == decision.get("implementation")):
            rows[-1] = {"block": block, "block_hash": rows[-1]["block_hash"],
                        "available": True, "source": "trabalho anterior com identidade conferida",
                        "implementation": decision["implementation"],
                        "code_sha256": hashlib.sha256(bytes.fromhex(current.removeprefix("0x"))).hexdigest(),
                        "code_bytes": len(current.removeprefix("0x")) // 2,
                        "total_supply_raw": history["supply_on_chain_raw"],
                        "roles_seen_active": {name: len(items) for name, items in
                                              roles["active_accounts_from_events"].items()},
                        "role_reads_incomplete": []}
    if not rows[-1].get("available") or rows[-1]["code_sha256"] != hashlib.sha256(bytes.fromhex(current.removeprefix("0x"))).hexdigest():
        raise ValueError("versão final diferente do código fixado")
    result = {"format": 1, "chain_id": CHAIN, "address": ADDRESS,
              "creation_block": history["creation_block"], "through_block": block,
              "upgrade_events": upgrades, "snapshots": rows, "candidate_accounts": len(accounts),
              "limit": "blocos de evento não distinguem operações diferentes na mesma transação ou bloco"}
    jwrite(job / "versions-raw.json", result)
    return result

if __name__ == "__main__":
    result = main(sys.argv[1])
    print(json.dumps({"versions": len(result["snapshots"]), "changes": len(result["upgrade_events"])}))
