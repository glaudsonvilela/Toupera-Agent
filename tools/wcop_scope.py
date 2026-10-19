"""Reproducible source comparison and measured ABI/test inventory for one wCOP implementation."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

SOLC = "/srv/toupera-agent/tools/solc-0.8.27"
CAST = "/home/toupera/.foundry/bin/cast"
ZERO = "0x" + "0" * 40

def private_json(path, data):
    path = Path(path)
    temp = path.with_name(path.name + ".tmp")
    temp.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    temp.chmod(0o600)
    temp.replace(path)

def sources_and_settings(info):
    if info.get("origin") != "Blockscout":
        raise ValueError("recompilação exata exige entradas completas do explorador")
    raw = info["source"]
    entries = [{"file_path": raw["file_path"], "source_code": raw["source_code"]}]
    entries.extend(raw.get("additional_sources") or [])
    sources = {}
    for item in entries:
        name, content = item.get("file_path"), item.get("source_code")
        if (not isinstance(name, str) or not isinstance(content, str) or not name or
                name.startswith("/") or "\\" in name or "\x00" in name or
                any(part in ("", ".", "..") for part in name.split("/")) or
                not name.endswith(".sol") or len(name) > 240 or
                len(content.encode("utf-8")) > 1_000_000 or name in sources):
            raise ValueError("arquivo de fonte inválido ou repetido")
        sources[name] = {"content": content}
    if not sources or len(sources) > 256 or sum(len(v["content"].encode("utf-8")) for v in sources.values()) > 10_000_000:
        raise ValueError("conjunto de fontes fora dos limites")
    settings = dict(raw.get("compiler_settings") or {})
    settings["outputSelection"] = {"*": {"": ["ast"]}, raw["file_path"]: {raw["name"]: [
        "abi", "evm.deployedBytecode.object", "evm.deployedBytecode.sourceMap", "evm.deployedBytecode.generatedSources", "evm.deployedBytecode.immutableReferences",
        "evm.methodIdentifiers", "storageLayout"]}}
    return {"language": "Solidity", "sources": sources, "settings": settings}

def mask_immutables(code, refs):
    if len(code) % 2:
        raise ValueError("runtime hexadecimal ímpar")
    value = bytearray.fromhex(code.removeprefix("0x"))
    for positions in refs.values():
        for part in positions:
            start, length = int(part["start"]), int(part["length"])
            if start + length > len(value):
                raise ValueError("referência imutável fora do runtime")
            value[start:start + length] = bytes(length)
    return bytes(value)

def compare_runtime(compiled, chain, refs):
    compiled = compiled.removeprefix("0x")
    chain = chain.removeprefix("0x")
    if not re.fullmatch(r"[0-9a-fA-F]*", compiled) or not re.fullmatch(r"[0-9a-fA-F]*", chain):
        return {"state": "indeterminado", "detail": "runtime contém ligações não resolvidas"}
    if len(compiled) != len(chain):
        return {"state": "diverge", "compiled_bytes": len(compiled)//2, "chain_bytes": len(chain)//2}
    if compiled.lower() == chain.lower():
        return {"state": "idêntico", "bytes": len(chain)//2}
    if mask_immutables(compiled, refs) == mask_immutables(chain, refs):
        return {"state": "confere sem campos imutáveis", "bytes": len(chain)//2,
                "immutable_locations": sum(len(v) for v in refs.values())}
    return {"state": "diverge", "bytes": len(chain)//2}

def compile_repro(job, info, chain_runtime):
    input_data = sources_and_settings(info)
    private_json(job / "compiler-input.json", input_data)
    proc = subprocess.run([SOLC, "--standard-json"], input=json.dumps(input_data), text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
    stdout = proc.stdout
    start = stdout.find("{")
    if start < 0:
        raise ValueError("compilador não devolveu JSON")
    output = json.loads(stdout[start:])
    private_json(job / "compiler-output.json", output)
    errors = [e.get("formattedMessage", e.get("message", ""))[:300] for e in output.get("errors", [])
              if e.get("severity") == "error"]
    if errors:
        return {"state": "falhou", "errors": errors[:5], "exit_code": proc.returncode}, None
    raw = info["source"]
    contract = (output.get("contracts") or {}).get(raw["file_path"], {}).get(raw["name"])
    if not contract:
        return {"state": "falhou", "errors": ["contrato principal ausente da saída"]}, None
    deployed = contract["evm"]["deployedBytecode"]
    comparison = compare_runtime(deployed["object"], chain_runtime, deployed.get("immutableReferences") or {})
    summary = {"state": comparison["state"], "comparison": comparison, "compiler": info["compiler"],
               "evm_version": input_data["settings"].get("evmVersion"),
               "sources": len(input_data["sources"]),
               "input_sha256": hashlib.sha256(json.dumps(input_data, sort_keys=True).encode()).hexdigest(),
               "storage_entries": len(contract.get("storageLayout", {}).get("storage", []))}
    return summary, contract

def cast_sig(signature):
    result = subprocess.run([CAST, "sig", signature], capture_output=True, text=True, timeout=5)
    if result.returncode or not re.fullmatch(r"0x[0-9a-f]{8}", result.stdout.strip()):
        raise ValueError("não foi possível calcular seletor")
    return result.stdout.strip()

def inventory(abi, engine, passed_tests):
    if isinstance(abi, str):
        abi = json.loads(abi)
    abi_functions = [entry for entry in abi if entry.get("type") == "function"]
    engine_selectors = {item.get("selector") for item in engine.get("selectors", [])}
    scenario_map = {
        "DEFAULT_ADMIN_ROLE()": ["testRoleGrantRevokeRenounce", "testReadSurface"],
        "MINTER_ROLE()": ["testReadSurface"],
        "PAUSER_ROLE()": ["testRoleGrantRevokeRenounce", "testReadSurface"],
        "UPGRADER_ROLE()": ["testReadSurface"],
        "UPGRADE_INTERFACE_VERSION()": ["testReadSurface"],
        "decimals()": ["testReadSurface"],
        "eip712Domain()": ["testReadSurface"],
        "getRoleAdmin(bytes32)": ["testRoleGrantRevokeRenounce"],
        "initialize(address,address,address,address,string,string)": ["testReinitializeRejected"],
        "name()": ["testReadSurface"],
        "paused()": ["testAuthorizedPauseUnpause"],
        "supportsInterface(bytes4)": ["testReadSurface"],
        "symbol()": ["testReadSurface"],
        "approve(address,uint256)": ["testApprovalDoesNotChangeSupply", "testAllowanceSpendAndReset"],
        "transfer(address,uint256)": ["testTransferRoundTrip", "testFuzzTransferConservesSupply"],
        "transferFrom(address,address,uint256)": ["testAllowanceSpendAndReset"],
        "mint(address,uint256)": ["testUnauthorizedMintCannotChangeSupply", "testAuthorizedMintBurnRoundTrip"],
        "burn(uint256)": ["testAuthorizedMintBurnRoundTrip"],
        "burnFrom(address,uint256)": ["testBurnFromWithAllowance"],
        "pause()": ["testUnauthorizedPauseIsRejected", "testAuthorizedPauseUnpause"],
        "unpause()": ["testAuthorizedPauseUnpause"],
        "grantRole(bytes32,address)": ["testRoleGrantRevokeRenounce"],
        "revokeRole(bytes32,address)": ["testRoleGrantRevokeRenounce"],
        "renounceRole(bytes32,address)": ["testRoleGrantRevokeRenounce"],
        "hasRole(bytes32,address)": ["testRoleGrantRevokeRenounce"],
        "balanceOf(address)": ["testTransferRoundTrip", "testFuzzTransferConservesSupply"],
        "totalSupply()": ["testApprovalDoesNotChangeSupply", "testAuthorizedMintBurnRoundTrip"],
        "allowance(address,address)": ["testAllowanceSpendAndReset", "testBurnFromWithAllowance"],
        "upgradeToAndCall(address,bytes)": ["testUnauthorizedUpgradeRejected"],
        "proxiableUUID()": ["testImplementationUUID"],
        "permit(address,address,uint256,uint256,uint8,bytes32,bytes32)": ["testPermitSignature"],
        "nonces(address)": ["testPermitSignature"],
        "DOMAIN_SEPARATOR()": ["testPermitSignature"],
    }
    functions = []
    for item in abi_functions:
        signature = item["name"] + "(" + ",".join(x["type"] for x in item.get("inputs", [])) + ")"
        selector = cast_sig(signature)
        mapped = scenario_map.get(signature, [])
        passed = [name for name in mapped if name in passed_tests]
        functions.append({"signature": signature, "selector": selector,
                          "mutability": item.get("stateMutability"),
                          "in_engine": selector in engine_selectors,
                          "scenarios_passed": passed,
                          "status": "cenário local" if passed else "sem cenário local"})
    functions.sort(key=lambda x: x["signature"])
    return {"abi_functions": len(functions), "engine_selectors": len(engine_selectors),
            "selectors_missing_in_engine": [x["signature"] for x in functions if not x["in_engine"]],
            "engine_only_selectors": sorted(engine_selectors - {x["selector"] for x in functions}),
            "scenario_functions": sum(bool(x["scenarios_passed"]) for x in functions),
            "functions": functions}

def roles_at_block(history, proxy, block, rpc):
    role_signatures = ["DEFAULT_ADMIN_ROLE()", "MINTER_ROLE()", "PAUSER_ROLE()", "UPGRADER_ROLE()"]
    roles = {}
    for name in role_signatures:
        selector = cast_sig(name)
        role = rpc("eth_call", [{"to": proxy, "data": selector}, hex(block)])
        roles[name.split("(")[0]] = role.lower()
    has_role = cast_sig("hasRole(bytes32,address)")
    events = sorted((entry for entry in history.get("timeline", [])
                     if entry.get("kind") in ("role_granted", "role_revoked")
                     and isinstance(entry.get("account"), str)
                     and isinstance(entry.get("role"), str)),
                    key=lambda entry: (entry["block"], entry["log_index"]))
    candidates = {entry["account"] for entry in events}
    event_state = {}
    for entry in events:
        pair = (entry["role"].lower(), entry["account"].lower())
        event_state[pair] = entry["kind"] == "role_granted"
    active = {}
    comparisons = []
    for name, role in roles.items():
        active[name] = []
        for account in sorted(candidates):
            data = has_role + role.removeprefix("0x").rjust(64, "0") + account.removeprefix("0x").rjust(64, "0")
            answer = rpc("eth_call", [{"to": proxy, "data": data}, hex(block)])
            current = int(answer, 16) != 0
            expected = event_state.get((role, account), False)
            comparisons.append({"role": name, "account": account,
                                "from_events": expected, "at_block": current,
                                "matches": expected == current})
            if current:
                active[name].append(account)
    complete_events = history.get("complete_event_range") is True
    return {"role_hashes": roles, "active_accounts_from_events": active,
            "candidate_count": len(candidates),
            "role_event_count": len(events), "event_replay_pairs": len(comparisons),
            "event_replay_matches_current": complete_events and all(x["matches"] for x in comparisons),
            "event_replay_differences": [x for x in comparisons if not x["matches"]],
            "event_replay_limit": "Só contas presentes em eventos; mudanças de implementação ou escrita sem evento podem deixar outras contas fora.",
            "complete_account_discovery": False}

def namespace_slots(info):
    raw = info.get("source") or {}
    entries = [{"file_path": raw.get("file_path"), "source_code": raw.get("source_code")}]
    entries.extend(raw.get("additional_sources") or [])
    found = []
    for item in entries:
        source = item.get("source_code") or ""
        for match in re.finditer(r"@custom:storage-location\s+(erc7201:[^\s*]+)", source):
            nearby = source[match.end():match.end()+1800]
            slot = re.search(r"bytes32\s+(?:private\s+)?constant\s+([A-Za-z_0-9]+)\s*=\s*(0x[0-9a-fA-F]{64})", nearby)
            if slot:
                found.append({"namespace": match.group(1), "constant": slot.group(1),
                              "slot": slot.group(2).lower(), "source": item.get("file_path")})
    return found

def parse_lcov(path):
    rows, current = [], None
    for line in Path(path).read_text().splitlines():
        if line.startswith("SF:"):
            current = {"file": line[3:]}
        elif current is not None and line.startswith(("LF:", "LH:", "BRF:", "BRH:", "FNF:", "FNH:")):
            key, value = line.split(":", 1)
            current[key] = int(value)
        elif line == "end_of_record" and current is not None:
            rows.append(current)
            current = None
    source_rows = [row for row in rows if not row["file"].startswith("test/")]
    totals = {key: sum(row.get(key, 0) for row in source_rows)
              for key in ("LF", "LH", "BRF", "BRH", "FNF", "FNH")}
    main = next((row for row in source_rows if row["file"].endswith("/LatamStable.sol")), None)
    active_rows = [row for row in source_rows if row.get("LH", 0) > 0]
    active_totals = {key: sum(row.get(key, 0) for row in active_rows)
                     for key in ("LF", "LH", "BRF", "BRH", "FNF", "FNH")}
    gaps = sorted(
        [{"file": row["file"], "lines_not_hit": row.get("LF",0)-row.get("LH",0),
          "branches_not_hit": row.get("BRF",0)-row.get("BRH",0)}
         for row in source_rows if row.get("LF",0)>row.get("LH",0) or row.get("BRF",0)>row.get("BRH",0)],
        key=lambda row: (-row["branches_not_hit"],-row["lines_not_hit"],row["file"]))
    active_gaps = sorted(
        [{"file": row["file"], "lines_not_hit": row.get("LF", 0)-row.get("LH", 0),
          "branches_not_hit": row.get("BRF", 0)-row.get("BRH", 0)}
         for row in active_rows if row.get("LF", 0)>row.get("LH", 0) or row.get("BRF", 0)>row.get("BRH", 0)],
        key=lambda row: (-row["branches_not_hit"], -row["lines_not_hit"], row["file"]))
    return {"measurement": "recompilação local da fonte, não o bytecode do bloco",
            "source_files": len(source_rows), "active_files": len(active_rows),
            "active_totals": active_totals, "totals": totals, "main_file": main,
            "largest_gaps": gaps[:12], "largest_active_gaps": active_gaps[:12],
            "all_files": source_rows}
