#!/usr/bin/env python3
"""One-contract site workflow. Read-only RPC, private files, bounded local tools."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from wcop_history import collect as collect_history
from wcop_scope import compile_repro, inventory, namespace_slots, parse_lcov, roles_at_block

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output" / "site-jobs"
ADDRESS = "0x8a1d45e102e886510e891d2ec656a708991e2d76"
RPC = "https://ethereum.publicnode.com"
CHAIN = 1
ENGINE = ROOT / "target/release/security-engine"
SLOT = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"

def save(path, content):
    path = Path(path)
    if path.is_symlink():
        path = path.resolve()
    temp = path.with_name(path.name + ".tmp")
    with open(temp, "w", encoding="utf-8") as file:
        file.write(content)
        file.flush()
        os.fsync(file.fileno())
    os.chmod(temp, 0o600)
    os.replace(temp, path)

def jread(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))

def jwrite(path, data):
    save(path, json.dumps(data, ensure_ascii=False, indent=2) + "\n")

def request_json(url, payload=None):
    headers = {"User-Agent": "Toupeira-Agent/1.0", "Accept": "application/json"}
    if payload is not None:
        headers["Content-Type"] = "application/json"
        payload = json.dumps(payload).encode()
    req = urllib.request.Request(url, payload, headers)
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as response:
                data = response.read(10_000_001)
            break
        except urllib.error.HTTPError as error:
            if error.code not in (429, 502, 503, 504) or attempt == 4:
                raise
            time.sleep(min(16, 2 ** (attempt + 1)))
    if len(data) > 10_000_000:
        raise ValueError("resposta excede limite")
    return json.loads(data)

def rpc(method, params):
    answer = request_json(RPC, {"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
    if "error" in answer:
        raise ValueError(str(answer["error"])[:150])
    return answer["result"]

def stage(job, name, detail):
    status = jread(job / "status.json")
    status["stage"] = name
    status["updated_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    status["events"].append({"time": status["updated_at"], "name": name, "detail": detail})
    jwrite(job / "status.json", status)

def source_info(job, address, runtime):
    try:
        data = request_json("https://sourcify.dev/server/v2/contract/1/" + address + "?fields=all")
        if data.get("runtimeMatch") == "match":
            compilation = data.get("compilation") or {}
            return {"state": "verificada no Sourcify", "compiler": compilation.get("compilerVersion"),
                    "name": compilation.get("name"), "files": len(data.get("sources") or {}),
                    "origin": "Sourcify", "source": data}
    except (urllib.error.URLError, ValueError, KeyError):
        pass
    try:
        data = request_json("https://eth.blockscout.com/api/v2/smart-contracts/" + address)
        deployed = data.get("deployed_bytecode")
        if data.get("is_verified") and isinstance(deployed, str) and deployed.lower() == runtime.lower():
            return {"state": "verificada no Blockscout", "compiler": data.get("compiler_version"),
                    "name": data.get("name"), "files": 1 + len(data.get("additional_sources") or []),
                    "origin": "Blockscout", "source": data}
    except (urllib.error.URLError, ValueError, KeyError):
        pass
    return {"state": "não obtida", "compiler": None, "name": None, "files": 0, "origin": None}

def command(args, path, timeout=90, cwd=None, env_extra=None):
    env = dict(os.environ)
    env["PATH"] = "/home/toupera/.local/bin:/home/toupera/.foundry/bin:/usr/local/bin:/usr/bin:/bin"
    if env_extra:
        env.update(env_extra)
    log_path = Path(path).resolve()
    job = next((parent for parent in log_path.parents if parent.parent == WORK), None)
    if job is None or not Path("/usr/bin/bwrap").is_file():
        raise RuntimeError("isolamento do executor indisponível")
    extra_mounts = []
    if Path(args[0]).name == "myth":
        scratch = job / "sandbox"
        mythril_cache = scratch / "mythril"
        if not mythril_cache.exists():
            shutil.copytree("/home/toupera/.mythril", mythril_cache)
        mpl_cache = scratch / "matplotlib"
        mpl_cache.mkdir(parents=True, exist_ok=True, mode=0o700)
        env["MPLCONFIGDIR"] = str(mpl_cache)
        extra_mounts = ["--bind", str(mythril_cache), "/home/toupera/.mythril"]
    isolated_args = ["/usr/bin/bwrap", "--ro-bind", "/", "/",
                     "--tmpfs", str(ROOT / "private-output"),
                     "--dir", str(WORK), "--dir", str(job),
                     "--bind", str(job), str(job),
                     "--dev-bind", "/dev", "/dev",
                     "--tmpfs", "/tmp",
                     "--unshare-pid", "--unshare-ipc", "--unshare-uts",
                     "--die-with-parent"] + extra_mounts + ["--"] + list(args)
    with open(path, "w", encoding="utf-8") as out:
        try:
            result = subprocess.run(isolated_args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                    stdout=out, stderr=subprocess.STDOUT, timeout=timeout)
            return result.returncode
        except subprocess.TimeoutExpired:
            out.write("\nTempo limite atingido.\n")
            return 124
        except OSError as error:
            out.write(str(error)[:180] + "\n")
            return 127

def engine_summary(data):
    return {name: len(data.get(name) or []) for name in
            ("instructions", "blocks", "selectors", "effects", "value_flows", "entry_summaries", "unresolved")}

def prepare_source(job, info):
    raw = info.get("source")
    if not raw:
        return None
    folder = job / "source"
    folder.mkdir(mode=0o700)
    if info["origin"] == "Blockscout":
        entries = [{"file_path": raw.get("file_path") or "Contract.sol", "source_code": raw.get("source_code")}]
        entries.extend(raw.get("additional_sources") or [])
        main = None
        for item in entries:
            name, content = item.get("file_path"), item.get("source_code")
            if not isinstance(name, str) or not isinstance(content, str):
                continue
            if name.startswith("/") or ".." in name.split("/") or len(name) > 240 or len(content) > 1_000_000:
                continue
            if not name.endswith(".sol"):
                continue
            aliases = [name]
            prefix = "lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/"
            if name.startswith(prefix):
                aliases.append("@openzeppelin/contracts/" + name[len(prefix):])
            prefix = "lib/openzeppelin-contracts-upgradeable/contracts/"
            if name.startswith(prefix):
                aliases.append("@openzeppelin/contracts-upgradeable/" + name[len(prefix):])
            for alias in aliases:
                path = folder.joinpath(*alias.split("/"))
                path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                save(path, content)
            if item is entries[0]:
                main = folder.joinpath(*name.split("/"))
        return main
    compilation = raw.get("compilation") or {}
    main_name = str(compilation.get("fullyQualifiedName", "")).split(":")[0]
    for name, item in (raw.get("sources") or {}).items():
        if not isinstance(name, str) or name.startswith("/") or ".." in name.split("/"):
            continue
        if not name.endswith(".sol") or len(name) > 180:
            continue
        content = item.get("content") if isinstance(item, dict) else None
        if not isinstance(content, str) or len(content) > 1_000_000:
            continue
        path = folder.joinpath(*name.split("/"))
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        save(path, content)
    main = folder.joinpath(*main_name.split("/"))
    return main if main.is_file() else None

def tool_line(name, state, detail):
    return {"name": name, "state": state, "detail": detail}

def run_tools(job, source, runtime_path, block_number, holder=None, role_actors=None, implementation=None):
    rows = []
    logs = job / "tools"
    logs.mkdir(mode=0o700)
    main = prepare_source(job, source)
    if main:
        stage(job, "Slither", "Compilando a fonte verificada e reunindo sinais.")
        output = job / "slither.json"
        code = command(["/home/toupera/.local/bin/slither", str(main), "--solc", str(ROOT / "tools/solc-0.8.27"),
                        "--solc-args", "--base-path .", "--fail-none", "--json", str(output)],
                       logs / "slither.log", 120, cwd=job / "source")
        try:
            result = jread(output)
            detectors = (result.get("results") or {}).get("detectors") or []
            signals = len(detectors)
            direct = [item for item in detectors if any(
                "LatamStable.sol" in str(element.get("source_mapping", {}).get("filename_relative", ""))
                for element in (item.get("elements") or []))]
            kinds = sorted({str(item.get("check")) for item in direct if item.get("check")})
            state = "executado" if result.get("success") else "falhou"
            detail = (str(signals) + " sinais brutos no conjunto; " + str(len(direct)) +
                      " com referência ao arquivo principal; tipos locais: " +
                      (", ".join(kinds[:8]) if kinds else "nenhum") +
                      "; compilador local 0.8.27; saída " + str(code))
            rows.append(tool_line("Slither", state, detail))
        except (OSError, ValueError):
            rows.append(tool_line("Slither", "falhou", "fonte não compilou ou não houve saída estruturada; saída " + str(code)))
    else:
        rows.append(tool_line("Slither", "pendente", "fonte compilável correspondente não disponível"))
    stage(job, "Mythril", "Lendo o runtime com limite de tempo.")
    myth = logs / "mythril.log"
    code = command(["/home/toupera/.local/bin/myth", "analyze", "-f", str(runtime_path),
                    "--bin-runtime", "--no-onchain-data", "--execution-timeout", "45", "-o", "json"],
                   myth, 65)
    try:
        report = json.loads(myth.read_text(encoding="utf-8").splitlines()[-1])
        issues = report.get("issues") or []
        titles = sorted({str(item.get("title")) for item in issues if item.get("title")})
        state = "executado" if report.get("success") else "falhou"
        detail = str(len(issues)) + " sinal(is) bruto(s); tipos: " + ", ".join(titles[:8]) + "; saída " + str(code)
    except (OSError, ValueError, IndexError):
        state = "limite" if code == 124 else "falhou"
        detail = "sem relatório estruturado; saída " + str(code)
    rows.append(tool_line("Mythril", state, detail))
    rows.extend([
        tool_line("Securify2", "não aplicável", "executor legado usa Solidity 0.5.12"),
        tool_line("Manticore", "não aplicável", "executor legado sem validação para este runtime"),
    ])
    stage(job, "Foundry", "Executando regras e sequências em cópia local do bloco.")
    foundry = job / "foundry"
    (foundry / "test").mkdir(parents=True, mode=0o700)
    shutil.copyfile(ROOT / "tools/wcop_readonly.t.sol", foundry / "test/Wcop.t.sol")
    save(foundry / "foundry.toml", '[profile.default]\nsolc = "/srv/toupera-agent/tools/solc-0.8.27"\ntest = "test"\noptimizer = true\n')
    forge_log = logs / "foundry.log"
    actors = (role_actors or {}).get("active_accounts_from_events") or {}
    full = bool(holder and implementation and all(actors.get(name) for name in
                ("DEFAULT_ADMIN_ROLE", "MINTER_ROLE", "PAUSER_ROLE", "UPGRADER_ROLE")))
    forge_args = ["/home/toupera/.foundry/bin/forge", "test", "--root", str(foundry),
                  "--fork-url", RPC, "--fork-block-number", str(block_number), "-vv"]
    selected_tests = None
    if not full:
        selected_tests = ("testUnauthorized|testApproval|testTransferRoundTrip|testAllowanceSpendAndReset"
                          if holder else "testUnauthorized|testApproval")
        forge_args.extend(["--match-test", selected_tests])
    env_extra = {"WCOP_HOLDER": holder} if holder else {}
    if full:
        env_extra.update({"WCOP_ADMIN": actors["DEFAULT_ADMIN_ROLE"][0],
                          "WCOP_MINTER": actors["MINTER_ROLE"][0],
                          "WCOP_PAUSER": actors["PAUSER_ROLE"][0],
                          "WCOP_UPGRADER": actors["UPGRADER_ROLE"][0],
                          "WCOP_IMPL": implementation})
    config = foundry / "foundry.toml"
    save(config, config.read_text() + "[fuzz]\nruns = 32\n")
    forge_code = command(forge_args, forge_log, 180, env_extra=env_extra)
    forge_text = forge_log.read_text(encoding="utf-8", errors="replace")
    for attempt in range(1, 3):
        transient_rpc = bool(re.search(r"HTTP error (?:429|502|503|504)|(?:429|502|503|504) (?:Bad Gateway|Service Unavailable|Gateway Timeout)", forge_text))
        if forge_code == 0 or not transient_rpc:
            break
        shutil.copyfile(forge_log, logs / ("foundry-attempt-" + str(attempt) + ".log"))
        time.sleep(2 ** attempt)
        forge_code = command(forge_args, forge_log, 180, env_extra=env_extra)
        forge_text = forge_log.read_text(encoding="utf-8", errors="replace")
    passed = re.search(r"(\d+) tests passed, (\d+) failed", forge_text)
    passed_names = sorted(set(re.findall(r"\[PASS\] (\w+)\(", forge_text)))
    available_tests = re.findall(r"\bfunction\s+(test[A-Za-z0-9_]+)\s*\(",
                                 (foundry / "test/Wcop.t.sol").read_text())
    expected = sum(bool(re.search(selected_tests, name)) for name in available_tests) if selected_tests else len(available_tests)
    jwrite(job / "scenarios.json", {"block": block_number, "full_role_context": full,
                                    "passed_names": passed_names, "expected": expected,
                                    "exit_code": forge_code})
    if forge_code == 0 and passed and int(passed.group(1)) == expected and passed.group(2) == "0":
        rows.append(tool_line("Foundry", "executado", str(expected) + " cenários locais passaram no bloco " +
                              str(block_number) + "; " +
                              ("papéis ativos, saldos, emissão, pausa, assinatura e sequências com 32 entradas"
                               if full else "escopo reduzido porque faltou um papel ativo") + "; cópia local"))
    else:
        rows.append(tool_line("Foundry", "limite" if forge_code == 124 else "falhou",
                              "cenários locais não concluídos; saída " + str(forge_code)))
    rows.extend([
        tool_line("Echidna", "pendente", "exige propriedade e harness de sequência"),
        tool_line("Medusa", "pendente", "exige propriedade e harness de sequência"),
        tool_line("Certora", "indisponível", "CLI sem chave; serviço externo não acionado"),
        tool_line("Halmos", "pendente", "exige propriedade Foundry curta"),
        tool_line("Kontrol", "pendente", "exige propriedade formal e premissas"),
    ])
    jwrite(job / "tools.json", rows)
    return rows

def deep_review(job, source, runtime, engine, roles):
    summary = {"compiler": None, "inventory": None, "coverage": None,
               "storage_namespaces": [], "errors": []}
    if not source or not source.get("source") or not runtime or not engine:
        summary["errors"].append("fonte, runtime ou mapa do motor indisponível")
        jwrite(job / "scope.json", summary)
        return summary
    stage(job, "Comparação", "Recompilando a fonte com as mesmas configurações.")
    contract = None
    try:
        comparison, contract = compile_repro(job, source, runtime)
        summary["compiler"] = comparison
    except (ValueError, KeyError, OSError, subprocess.TimeoutExpired) as error:
        summary["errors"].append("comparação: " + type(error).__name__ + ": " + str(error)[:120])
    summary["storage_namespaces"] = namespace_slots(source)
    slots = [item["slot"] for item in summary["storage_namespaces"]]
    summary["storage_slots_unique"] = len(slots) == len(set(slots))
    scenarios = jread(job / "scenarios.json") if (job / "scenarios.json").exists() else {}
    stage(job, "Mapa", "Ligando entradas públicas aos cenários concluídos.")
    try:
        abi = contract.get("abi") if contract else (source.get("source") or {}).get("abi")
        if abi:
            summary["inventory"] = inventory(abi, engine, set(scenarios.get("passed_names") or []))
            jwrite(job / "function-map.json", summary["inventory"])
    except (ValueError, OSError, KeyError) as error:
        summary["errors"].append("mapa: " + type(error).__name__ + ": " + str(error)[:120])
    if (job / "source/src/LatamStable.sol").is_file():
        stage(job, "Cobertura", "Medindo linhas e caminhos na recompilação local.")
        project = job / "coverage"
        project.mkdir(mode=0o700)
        for folder in ("src", "lib", "@openzeppelin"):
            origin = job / "source" / folder
            if origin.is_dir():
                shutil.copytree(origin, project / folder)
        (project / "test").mkdir(mode=0o700)
        shutil.copyfile(ROOT / "tools/wcop_coverage.t.sol", project / "test/WcopCoverage.t.sol")
        save(project / "foundry.toml",
             '[profile.default]\nsrc = "src"\ntest = "test"\n'
             'solc = "/srv/toupera-agent/tools/solc-0.8.27"\n'
             'auto_detect_remappings = false\n'
             'remappings = ["@openzeppelin/contracts/=./@openzeppelin/contracts/", '
             '"@openzeppelin/contracts-upgradeable/=./@openzeppelin/contracts-upgradeable/"]\n'
             'dynamic_test_linking = false\n'
             '[fuzz]\nruns = 32\n')
        lcov = project / "coverage.info"
        log = job / "tools/coverage.log"
        code = command(["/home/toupera/.foundry/bin/forge", "coverage", "--root", str(project),
                        "--report", "lcov", "--report-file", str(lcov)], log, 180)
        if code == 0 and lcov.is_file():
            summary["coverage"] = parse_lcov(lcov)
            summary["coverage"]["local_tests_passed"] = len(re.findall(
                r"\[PASS\]", log.read_text(encoding="utf-8", errors="replace")))
            jwrite(job / "coverage.json", summary["coverage"])
        else:
            summary["errors"].append("cobertura local não concluiu; saída " + str(code))
    if (job / "coverage/src/LatamStable.sol").is_file():
        stage(job, "Sequências", "Rodando duas campanhas curtas na cópia local.")
        project = job / "coverage"
        shutil.copyfile(ROOT / "tools/wcop_properties.sol", project / "src/WcopProperties.sol")
        echidna_log = job / "tools/echidna.log"
        echidna_code = command(["/usr/local/bin/echidna", ".", "--contract", "WCOPProperties",
                                "--crytic-args", "--foundry-compile-all", "--workers", "2",
                                "--test-limit", "200", "--seq-len", "20", "--timeout", "30",
                                "--format", "text", "--disable-slither"], echidna_log, 60, cwd=project)
        echidna_text = echidna_log.read_text(encoding="utf-8", errors="replace")
        echidna_ok = (echidna_code == 0 and
                      "echidna_supply_matches: passing" in echidna_text and
                      "echidna_balances_match: passing" in echidna_text)
        medusa_log = job / "tools/medusa.log"
        medusa_code = command(["/usr/local/bin/medusa", "fuzz", "--compilation-target", ".",
                               "--target-contracts", "WCOPProperties", "--timeout", "20",
                               "--test-limit", "200", "--seq-len", "20", "--workers", "2",
                               "--no-color"], medusa_log, 55, cwd=project)
        medusa_text = medusa_log.read_text(encoding="utf-8", errors="replace")
        medusa_ok = (medusa_code == 0 and
                     "[PASSED] Assertion Test: WCOPProperties.echidna_supply_matches()" in medusa_text and
                     "[PASSED] Assertion Test: WCOPProperties.echidna_balances_match()" in medusa_text and
                     "0 test(s) failed" in medusa_text)
        summary["fuzzers"] = {"Echidna": {"state": "executado" if echidna_ok else "incompleto",
                                          "exit_code": echidna_code, "properties_passed": 2 if echidna_ok else 0},
                              "Medusa": {"state": "executado" if medusa_ok else "incompleto",
                                         "exit_code": medusa_code, "properties_passed": 2 if medusa_ok else 0}}
        stage(job, "Prova", "Tentando uma propriedade limitada na cópia local.")
        shutil.copyfile(ROOT / "tools/wcop_formal.t.sol", project / "test/WcopFormal.t.sol")
        halmos_log = job / "tools/halmos.log"
        halmos_code = command(["/home/toupera/.local/bin/halmos", "--root", str(project),
                               "--contract", "WCOPFormal", "--function", "check_",
                               "--solver-timeout-branching", "10000", "--solver-timeout-assertion", "10000"],
                              halmos_log, 45, cwd=project)
        halmos_text = halmos_log.read_text(encoding="utf-8", errors="replace")
        halmos_ok = halmos_code == 0 and re.search(r"Symbolic test result: 3 passed; 0 failed", halmos_text)
        summary["formal"] = {"state": "concluído" if halmos_ok else "inconclusivo",
                             "exit_code": halmos_code, "properties_passed": 3 if halmos_ok else 0,
                             "reason": "" if halmos_ok else
                             ("limite do executor no preparo local" if "Unsupported cheat code" in halmos_text
                              else "prova não concluída no tempo ou com este harness")}
    jwrite(job / "scope.json", summary)
    return summary

def snapshot_manifest(job, address, block_number, block_hash, implementation, slot_word):
    names = ("smart.md", "runtime.hex", "implementation.hex", "engine.json",
             "engine-impl.json", "compiler-input.json", "compiler-output.json",
             "scope.json", "history.json", "roles.json", "tools.json",
             "trace.json", "trace-reconcile.json", "scenarios.json", "coverage.json",
             "adaptive.json", "adaptive-state.json", "adaptive-trace.json",
             "adaptive-reconcile.json")
    artifacts = {}
    for name in names:
        path = job / name
        if path.is_file() and not path.is_symlink():
            data = path.read_bytes()
            artifacts[name] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    manifest = {"format": 1, "chain_id": CHAIN, "address": address,
                "block_number": block_number, "block_hash": block_hash,
                "implementation": implementation, "eip1967_slot_word": slot_word,
                "artifacts": artifacts,
                "engine_binary_sha256": hashlib.sha256(ENGINE.read_bytes()).hexdigest(),
                "solc_binary_sha256": hashlib.sha256((ROOT / "tools/solc-0.8.27").read_bytes()).hexdigest()}
    jwrite(job / "job-manifest.json", manifest)
    return manifest

def worker(job_id):
    job = WORK / job_id
    try:
        submitted = (job / "smart.md").read_text(encoding="utf-8")
        match = re.search(r"^- Endereço: (0x[0-9a-fA-F]{40})$", submitted, re.MULTILINE)
        if not match or match.group(1).lower() != ADDRESS:
            raise ValueError("smart.md não contém o endereço esperado")
        address = match.group(1).lower()
        stage(job, "Rede", "Conferindo o identificador e fixando um bloco.")
        if int(rpc("eth_chainId", []), 16) != CHAIN:
            raise ValueError("a rede respondeu com outro identificador")
        block = rpc("eth_getBlockByNumber", ["latest", False])
        block_number = int(block["number"], 16)
        block_hash = block["hash"]
        tag = hex(block_number)
        runtime = rpc("eth_getCode", [address, tag])
        if not re.fullmatch(r"0x(?:[0-9a-fA-F]{2})+", runtime):
            raise ValueError("contrato sem código neste bloco")
        runtime_path = job / "runtime.hex"
        save(runtime_path, runtime + "\n")
        runtime_hash = hashlib.sha256(bytes.fromhex(runtime[2:])).hexdigest()
        stage(job, "Fonte", "Consultando fontes verificadas para o endereço escolhido.")
        source = source_info(job, address, runtime)
        word = rpc("eth_getStorageAt", [address, SLOT, tag])
        implementation = "0x" + word[-40:].lower() if isinstance(word, str) and len(word) == 66 else None
        if implementation == "0x" + "0" * 40:
            implementation = None
        impl_runtime = None
        impl_source = None
        impl_runtime_path = None
        if implementation:
            impl_runtime = rpc("eth_getCode", [implementation, tag])
            if re.fullmatch(r"0x(?:[0-9a-fA-F]{2})+", impl_runtime):
                impl_runtime_path = job / "implementation.hex"
                save(impl_runtime_path, impl_runtime + "\n")
                impl_source = source_info(job, implementation, impl_runtime)
        stage(job, "Motor Rust", "Reconstruindo runtime principal e implementação identificada.")
        code = command([str(ENGINE), "--bytecode", str(runtime_path)], job / "engine.json", 90)
        if code:
            raise ValueError("motor Rust não concluiu o contrato principal")
        main_engine = jread(job / "engine.json")
        summary = engine_summary(main_engine)
        impl_summary = None
        if impl_runtime_path:
            code = command([str(ENGINE), "--bytecode", str(impl_runtime_path)], job / "engine-impl.json", 90)
            if code == 0:
                impl_engine = jread(job / "engine-impl.json")
                impl_summary = engine_summary(impl_engine)
        stage(job, "Histórico", "Lendo eventos do endereço desde sua criação até o bloco fixado.")
        history = None
        history_error = None
        try:
            history = collect_history(address, block_number, request_json, rpc)
            jwrite(job / "history.json", history)
        except (ValueError, KeyError, OSError, urllib.error.URLError) as error:
            history_error = type(error).__name__ + ": " + str(error)[:120]
            save(job / "history-error.txt", history_error + "\n")
        sift = None
        if history and impl_summary:
            stage(job, "Peneira Rust", "Ordenando dúvidas técnicas do bytecode.")
            sift_code = command([str(ENGINE), "--wcop-sift", str(job / "engine-impl.json"),
                                 str(job / "history.json")], job / "sift.json", 30)
            if sift_code == 0:
                sift = jread(job / "sift.json")
        result = (
            "# Resultado do motor Rust\n\n"
            + "- Contrato: " + address + "\n"
            + "- Rede: Ethereum (1)\n"
            + "- Bloco: " + str(block_number) + " / " + block_hash + "\n"
            + "- Hash SHA-256 do runtime: " + runtime_hash + "\n"
            + "- Fonte do endereço: " + source["state"] + "\n"
            + "- Implementação no bloco: " + str(implementation or "não identificada") + "\n\n"
            + "## Estrutura do endereço\n\n"
            + "\n".join("- " + key + ": " + str(value) for key, value in summary.items()) + "\n")
        if implementation:
            result += "\n## Implementação\n\n- Endereço: " + implementation + "\n"
            result += "- Fonte: " + (impl_source or {}).get("state", "não obtida") + "\n"
            if impl_runtime:
                result += "- Hash SHA-256 do runtime: " + hashlib.sha256(bytes.fromhex(impl_runtime[2:])).hexdigest() + "\n"
            if impl_summary:
                result += "\n".join("- " + key + ": " + str(value) for key, value in impl_summary.items()) + "\n"
                selectors = [item.get("selector") for item in impl_engine.get("selectors", []) if item.get("selector")]
                result += "- Seletores encontrados: " + ", ".join(selectors[:40]) + "\n"
            if impl_source:
                result += "- Compilador declarado: " + str(impl_source.get("compiler") or "indisponível") + "\n"
                result += "- Arquivos de fonte: " + str(impl_source.get("files", 0)) + "\n"
                raw_abi = (impl_source.get("source") or {}).get("abi") or []
                if isinstance(raw_abi, str):
                    try:
                        raw_abi = json.loads(raw_abi)
                    except ValueError:
                        raw_abi = []
                names = sorted({item.get("name") for item in raw_abi if isinstance(item, dict) and item.get("type") == "function" and item.get("name")})
                if names:
                    result += "- Funções públicas na ABI: " + ", ".join(names[:45]) + "\n"
        if history:
            result += ("\n## Linha do tempo e oferta\n\n"
                       "- Criação: bloco " + str(history["creation_block"]) + ", transação " + history["creation_tx"] + "\n"
                       "- Intervalo de eventos: " + str(history["creation_block"]) + " a " + str(block_number) +
                       "; " + str(history["event_count"]) + " eventos, " + str(history["pages"]) + " página(s) da fonte.\n"
                       "- Oferta emitida (unidades brutas): " + history["minted_raw"] + "\n"
                       "- Oferta retirada (unidades brutas): " + history["burned_raw"] + "\n"
                       "- Oferta observada no bloco (unidades brutas): " + history["supply_on_chain_raw"] + "\n"
                       "- Balanço entre eventos e oferta: " + ("confere" if history["supply_matches_events"] else "diverge; revisar") + "\n")
            result += "- Tipos de evento: " + ", ".join(
                name + "=" + str(count) for name, count in sorted(history["event_counts"].items())) + "\n"
            upgrades = [event for event in history["timeline"] if event["kind"] == "upgraded"]
            if upgrades and implementation:
                result += "- Última mudança de implementação confere com o slot no bloco: " + (
                    "sim" if upgrades[-1].get("implementation") == implementation else "não; revisar") + "\n"
            for event in history["timeline"][:18]:
                result += "- " + event["kind"] + ": bloco " + str(event["block"]) + ", tx " + event["tx"] + ", log " + str(event["log_index"])
                if event.get("implementation"):
                    result += ", implementação " + event["implementation"]
                if event.get("role"):
                    result += ", papel " + event["role"]
                if event.get("account"):
                    result += ", conta " + event["account"]
                result += "\n"
            important = [item for item in history["movements"] if item["flow"] in ("mint", "burn")]
            result += "- Movimentos de oferta: " + str(len(important)) + " registro(s).\n"
            for event in important[:12]:
                result += ("- " + event["flow"] + ": " + event["amount_raw"] + " unidades brutas; bloco " +
                           str(event["block"]) + ", tx " + event["tx"] + ", log " + str(event["log_index"]) + "\n")
            result += "- Registros completos: history.json privado do trabalho; o texto acima é um resumo.\n"
        else:
            result += "\n## Histórico\n\n- Leitura incompleta: " + str(history_error) + "\n"
        if sift:
            result += ("\n## Peneira do motor Rust\n\n"
                       "- Dúvidas iniciais: " + str(sift["input_uncertainties"]) + "\n"
                       "- Contexto de versão da rede separado: " + str(sift["environment_context_count"]) + "\n"
                       "- Pontos ordenados para revisão: " + str(sift["selected_count"]) + "\n"
                       "- A ordem indica onde revisar; não mede chance de falha.\n")
            for item in sift["selected"][:12]:
                result += "- PC " + str(item["pc"]) + ": " + item["question"] + "\n"
        save(ROOT / "result_smart.md", result)
        save(job / "result_smart.md", result)
        stage(job, "Ferramentas", "Executando leituras compatíveis e registrando as demais.")
        selected_source = impl_source if impl_source and impl_source.get("source") else source
        selected_runtime = impl_runtime_path or runtime_path
        roles = None
        if history:
            stage(job, "Papéis", "Conferindo no bloco as contas vistas no histórico.")
            try:
                roles = roles_at_block(history, address, block_number, rpc)
                jwrite(job / "roles.json", roles)
            except (ValueError, OSError, urllib.error.URLError) as error:
                save(job / "roles-error.txt", type(error).__name__ + ": " + str(error)[:120] + "\n")
        rows = run_tools(job, selected_source, selected_runtime, block_number,
                         (history.get("funded_holder") or {}).get("address") if history else None,
                         roles, implementation)
        deep = deep_review(job, impl_source, impl_runtime, impl_engine if impl_summary else None, roles)
        trace_review = None
        adaptive_summary = None
        adaptive_review = None
        if history and roles and impl_summary:
            stage(job, "Caminhos", "Lendo PCs da implementação em cópia local do bloco.")
            try:
                local_trace_code = command([sys.executable, str(ROOT / "tools/wcop_trace.py"), job_id],
                                           job / "tools/trace.log", 180)
                if local_trace_code != 0 or not (job / "trace.json").exists():
                    raise RuntimeError("leitura local dos caminhos não concluiu")
                trace_code = command([str(ENGINE), "--wcop-trace", str(job / "engine-impl.json"),
                                      str(job / "trace.json")], job / "trace-reconcile.json", 45)
                if trace_code == 0:
                    trace_review = jread(job / "trace-reconcile.json")
                else:
                    save(job / "trace-error.txt", "motor não concluiu a ligação dos caminhos\n")
            except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired) as error:
                save(job / "trace-error.txt", type(error).__name__ + ": " + str(error)[:160] + "\n")
        if trace_review and trace_review.get("trace_errors") == 0:
            stage(job, "Ciclo", "Escolhendo a próxima operação pelo ganho observado na cópia local.")
            adaptive_code = command([sys.executable, str(ROOT / "tools/wcop_adaptive.py"), job_id],
                                    job / "tools/adaptive.log", 270)
            if adaptive_code == 0 and (job / "adaptive-trace.json").is_file():
                review_code = command([str(ENGINE), "--wcop-trace",
                                       str(job / "engine-impl.json"),
                                       str(job / "adaptive-trace.json")],
                                      job / "adaptive-reconcile.json", 45)
                if review_code == 0:
                    adaptive_summary = jread(job / "adaptive.json")
                    adaptive_review = jread(job / "adaptive-reconcile.json")
                else:
                    save(job / "adaptive-error.txt", "ligação dos caminhos não concluiu\n")
            else:
                save(job / "adaptive-error.txt", "ciclo local não concluiu; saída " +
                     str(adaptive_code) + "\n")
        for item in rows:
            if item["name"] in ("Echidna", "Medusa"):
                state = (deep.get("fuzzers") or {}).get(item["name"])
                if state:
                    item["state"] = state["state"]
                    item["detail"] = ("duas regras de oferta/saldos em sequências locais, alvo de 200 chamadas; "
                                      "saída " + str(state["exit_code"]))
            if item["name"] == "Halmos" and deep.get("formal"):
                item["state"] = deep["formal"]["state"]
                item["detail"] = "três propriedades locais de oferta, transferência e pausa: " + (
                    "concluída" if deep["formal"]["state"] == "concluído" else deep["formal"]["reason"])
        jwrite(job / "tools.json", rows)
        stage(job, "Identidade", "Fixando entradas e conferindo vínculos da leitura.")
        final_block = rpc("eth_getBlockByNumber", [tag, False])
        if not isinstance(final_block, dict) or final_block.get("hash", "").lower() != block_hash.lower():
            raise ValueError("o bloco fixado mudou durante a leitura")
        snapshot_manifest(job, address, block_number, block_hash, implementation, word)
        guard_code = command([str(ENGINE), "--wcop-guard", str(job)],
                             job / "decision.json", 60)
        if guard_code == 0:
            decision = jread(job / "decision.json")
        else:
            decision = {"state": "inconclusivo", "source_binding": "indisponível",
                        "identity_ok": False, "claims": [],
                        "reasons": ["guardião não concluiu; nenhuma conclusão elevada"]}
            jwrite(job / "decision.json", decision)
        linked_source = (decision.get("identity_ok") is True and
                         decision.get("source_binding") in ("exact", "masked_immutables"))
        if not linked_source:
            deep = {"compiler": {"state": "suspensa"},
                    "errors": ["relações de fonte suspensas pelo guardião"]}
        raw_result = result.split("\n", 1)[1] if "\n" in result else result
        if not linked_source:
            raw_result = "\n".join(line for line in raw_result.splitlines()
                                   if not line.startswith(("- Fonte do endereço:", "- Fonte:",
                                                           "- Compilador declarado:", "- Arquivos de fonte:",
                                                           "- Funções públicas na ABI:")))
        final = "# Resultado do contrato\n\n## Decisão das evidências\n\n"
        final += "- Estado: " + str(decision.get("state", "inconclusivo")) + ".\n"
        final += "- Fonte ligada ao runtime: " + str(decision.get("source_binding", "indisponível")) + ".\n"
        for claim in decision.get("claims", []):
            final += ("- " + str(claim.get("title", claim.get("id", "item"))) + ": " +
                      str(claim.get("state", "inconclusivo")) + "; " +
                      str(claim.get("reason", "sem motivo")) + ".\n")
        for reason in decision.get("reasons", []):
            final += "- Limite: " + str(reason) + ".\n"
        final += ("\n## Dados coletados\n\n- Observações brutas do trabalho, sujeitas aos estados acima.\n" +
                  raw_result + "\n")
        final += "\n## Ferramentas\n\n"
        for item in rows:
            final += "- " + item["name"] + " — " + item["state"] + ": " + item["detail"] + ".\n"
        final += "\n## Mapa e medição\n\n"
        compiler = deep.get("compiler") or {}
        final += "- Comparação da fonte compilada: " + str(compiler.get("state") or "incompleta") + ".\n"
        final += "- Compilador: " + str(compiler.get("compiler") or "indisponível") + "; EVM: " + str(compiler.get("evm_version") or "indisponível") + ".\n"
        final += "- Espaços de dados nomeados na fonte: " + str(len(deep.get("storage_namespaces") or [])) + "; posições distintas: " + ("sim" if deep.get("storage_slots_unique") else "não confirmado") + ".\n"
        if roles:
            final += "- Contas com papel ativo entre as vistas no histórico: " + ", ".join(
                name + "=" + str(len(accounts)) for name, accounts in
                roles.get("active_accounts_from_events", {}).items()) + ".\n"
        inv = deep.get("inventory") or {}
        if inv:
            final += ("- Funções públicas na ABI: " + str(inv["abi_functions"]) +
                      "; entradas identificadas pelo Rust: " + str(inv["engine_selectors"]) +
                      "; funções com ao menos um cenário local concluído: " +
                      str(inv["scenario_functions"]) + ".\n")
            if inv["selectors_missing_in_engine"] or inv["engine_only_selectors"]:
                final += "- Divergências de seletores: ABI sem motor " + str(inv["selectors_missing_in_engine"]) + "; motor sem ABI " + str(inv["engine_only_selectors"]) + ".\n"
            missing = [item["signature"] for item in inv["functions"] if item["status"] != "cenário local"]
            final += "- Entradas sem cenário local: " + (", ".join(missing) if missing else "nenhuma na ABI") + ".\n"
            final += "- Lista por função: function-map.json privado do trabalho.\n"
        cov = deep.get("coverage") or {}
        if cov:
            totals = cov["totals"]
            main = cov.get("main_file") or {}
            final += ("- Fonte principal, medição local: " + str(main.get("LH", 0)) + "/" +
                      str(main.get("LF", 0)) + " linhas; " + str(main.get("BRH", 0)) + "/" +
                      str(main.get("BRF", 0)) + " ramos medidos.\n")
            final += ("- Fonte principal e bibliotecas usadas na compilação: " +
                      str(totals["LH"]) + "/" + str(totals["LF"]) + " linhas; " +
                      str(totals["BRH"]) + "/" + str(totals["BRF"]) + " ramos medidos.\n")
            active = cov.get("active_totals") or {}
            if active:
                final += ("- Arquivos com linhas executadas nesta rodada (" + str(cov.get("active_files", 0)) +
                          "): " + str(active["LH"]) + "/" + str(active["LF"]) +
                          " linhas; " + str(active["BRH"]) + "/" + str(active["BRF"]) +
                          " ramos. Esta seleção depende dos cenários, não define todos os caminhos alcançáveis.\n")
            final += ("- A soma das bibliotecas inclui caminhos que podem não ser alcançáveis por este token; "
                      "ela não é uma porcentagem do runtime publicado.\n")
            final += ("- Ensaios sobre a fonte recompilada: " + str(cov.get("local_tests_passed", 0)) +
                      " concluídos; esta medição local não mede diretamente o bytecode do bloco.\n")
            for gap in (cov.get("largest_active_gaps") or [])[:6]:
                final += ("- Caminho ativo com lacuna: " + gap["file"] + " — " +
                          str(gap["lines_not_hit"]) + " linhas e " +
                          str(gap["branches_not_hit"]) + " ramos sem passagem.\n")
            for gap in cov["largest_gaps"][:3]:
                final += ("- Arquivo incluído com lacuna: " + gap["file"] + " — " +
                          str(gap["lines_not_hit"]) + " linhas e " +
                          str(gap["branches_not_hit"]) + " ramos sem passagem.\n")
        for name, item in (deep.get("fuzzers") or {}).items():
            final += ("- " + name + ": " + item["state"] + "; regras concluídas " +
                      str(item["properties_passed"]) + "/2 em campanha local limitada.\n")
        if deep.get("formal"):
            final += ("- Halmos: " + deep["formal"]["state"] + "; " +
                      (deep["formal"]["reason"] or str(deep["formal"].get("properties_passed", 0)) + " propriedades locais concluídas") + ".\n")
        if trace_review:
            final += ("- Bytecode na cópia local: " + str(trace_review["observed_instruction_pcs"]) +
                      "/" + str(trace_review["disassembled_instruction_pcs"]) +
                      " PCs percorridos em " + str(trace_review["calls"]) + " chamadas locais; " +
                      str(trace_review["observed_blocks"]) + "/" + str(trace_review["engine_blocks"]) +
                      " blocos do mapa visitados.\n")
            final += ("- Ligações entre blocos: " + str(trace_review["matched_static_edges"]) +
                      " presentes no mapa; " + str(trace_review["unmatched_transitions"]) +
                      " transições observadas ainda fora do mapa estático. Dúvidas em PCs visitados: " +
                      str(trace_review["observed_uncertainties"]) + ".\n")
            final += ("- A leitura reúne chamadas isoladas e uma sequência local; PCs restantes podem aparecer em outros estados.\n")
        if adaptive_summary and adaptive_review:
            final += ("- Ciclo orientado por resultados: " + str(adaptive_summary["attempts"]) +
                      " escolhas locais; " + str(adaptive_summary["new_pcs"]) +
                      " PCs e " + str(adaptive_summary["new_edges"]) +
                      " transições novas além da leitura inicial; estado " +
                      str(adaptive_summary["status"]) + ".\n")
            final += ("- Sequências curtas na cópia local: " + str(adaptive_summary.get("setup_attempts", 0)) +
                      "; " + str(adaptive_summary.get("new_blocks", 0)) +
                      " blocos novos e " + str(adaptive_summary.get("frontier_crossings", 0)) +
                      " passagens de bloco visto para bloco novo.\n")
            final += ("- Diferenças a revisar no ciclo: " +
                      str(len(adaptive_summary.get("deviations") or [])) +
                      "; nenhuma é conclusão automática. Arquivos detalhados no VPS privado.\n")
        elif (job / "adaptive-error.txt").is_file():
            final += "- Ciclo orientado por resultados: inconclusivo nesta rodada.\n"
        for error in deep.get("errors") or []:
            final += "- Limite: " + error + ".\n"
        final += ("- Ainda sem cobertura total de caminhos e estados; uma função exercitada uma vez pode ter "
                  "outros resultados em condições diferentes.\n"
                  "- A troca local de implementação foi ensaiada; compatibilidade com versões futuras e relações com outros "
                  "contratos seguem fora desta medição.\n")
        final += ("\n## Interpretação\n\n"
                  "- Estes números descrevem a estrutura observada; não demonstram uma falha.\n"
                  "- Sinais brutos precisam de reprodução local e revisão para sustentar uma conclusão.\n"
                  "- As etapas pendentes indicam exatamente o que falta configurar para aprofundar esta análise.\n"
                  "- A lista de endereços veio do usuário; confira as regras atuais do programa antes de qualquer envio.\n")
        save(ROOT / "end_smart_result.md", final)
        save(job / "end_smart_result.md", final)
        stage(job, "Concluído", "Relatório final disponível no painel.")
        status = jread(job / "status.json")
        status["state"] = "complete"
        jwrite(job / "status.json", status)
    except Exception as error:
        message = type(error).__name__ + ": " + str(error)[:160]
        stage(job, "Interrompido", message)
        final = "# Resultado do contrato\n\n- Contrato: " + ADDRESS + "\n- Estado: incompleto\n- Motivo: " + message + "\n"
        save(ROOT / "end_smart_result.md", final)
        save(job / "end_smart_result.md", final)
        status = jread(job / "status.json")
        status["state"] = "failed"
        jwrite(job / "status.json", status)

def current_status(job):
    status = jread(job / "status.json")
    if status.get("state") != "running":
        return status
    pid_path = job / "worker.pid"
    missing = False
    if pid_path.exists():
        try:
            os.kill(int(pid_path.read_text().strip()), 0)
        except (OSError, ValueError):
            missing = True
    old = time.time() - (job / "status.json").stat().st_mtime > 600
    if missing or old:
        reason = "O processo terminou antes do relatório." if missing else "Tempo máximo do trabalho atingido."
        stage(job, "Interrompido", reason)
        status = jread(job / "status.json")
        status["state"] = "failed"
        jwrite(job / "status.json", status)
        message = "# Resultado do contrato\n\n- Estado: incompleto\n- Motivo: " + reason + "\n"
        save(ROOT / "end_smart_result.md", message)
        save(job / "end_smart_result.md", message)
    return status

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def reply(self, code, data):
        body = json.dumps(data, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        if self.path != "/api/smart/latest":
            return self.reply(404, {"error": "não encontrado"})
        active = WORK / "active.json"
        if not active.exists():
            return self.reply(200, {"state": "idle", "events": []})
        job = WORK / jread(active)["id"]
        status = current_status(job)
        if (job / "end_smart_result.md").is_file():
            status["report"] = (job / "end_smart_result.md").read_text(encoding="utf-8")
        return self.reply(200, status)
    def do_POST(self):
        if self.path != "/api/smart":
            return self.reply(404, {"error": "não encontrado"})
        if self.headers.get("Origin") != "https://toupeiraagente.tech":
            return self.reply(403, {"error": "origem inválida"})
        if not self.headers.get("Content-Type", "").startswith("application/json"):
            return self.reply(415, {"error": "formato inválido"})
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if not 0 < size <= 512:
                return self.reply(413, {"error": "entrada inválida"})
            payload = json.loads(self.rfile.read(size))
            address = str(payload.get("address", "")).lower()
            if address != ADDRESS:
                return self.reply(400, {"error": "esta etapa está configurada somente para wCOP"})
            with open(WORK / ".lock", "a+") as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                active = WORK / "active.json"
                if active.exists():
                    prior = WORK / jread(active)["id"]
                    state = current_status(prior)
                    if state["state"] == "running":
                        return self.reply(409, {"error": "há uma leitura em andamento"})
                job_id = uuid.uuid4().hex[:16]
                job = WORK / job_id
                job.mkdir(mode=0o700)
                now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
                status = {"id": job_id, "address": ADDRESS, "chain": "Ethereum", "state": "running",
                          "stage": "Recebido", "updated_at": now,
                          "events": [{"time": now, "name": "Recebido", "detail": "Endereço registrado no VPS."}]}
                jwrite(job / "status.json", status)
                jwrite(active, {"id": job_id})
                input_md = ("# Entrada do contrato\n\n- Trabalho: " + job_id +
                            "\n- Endereço: " + ADDRESS + "\n- Nome: wCOP / peso colombiano" +
                            "\n- Rede: Ethereum (1)\n- Modo: leitura da rede e processamento local.\n")
                save(ROOT / "smart.md", input_md)
                save(job / "smart.md", input_md)
                for old in ("result_smart.md", "end_smart_result.md"):
                    old_path = ROOT / old
                    (old_path.resolve() if old_path.is_symlink() else old_path).unlink(missing_ok=True)
                with open(job / "worker.log", "w") as log:
                    process = subprocess.Popen([sys.executable, __file__, "worker", job_id],
                                               stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                               start_new_session=True)
                    save(job / "worker.pid", str(process.pid) + "\n")
            return self.reply(202, status)
        except (ValueError, TypeError, json.JSONDecodeError):
            return self.reply(400, {"error": "entrada inválida"})
        except Exception:
            return self.reply(500, {"error": "não foi possível iniciar"})

def recover_active():
    active = WORK / "active.json"
    if not active.exists():
        return
    try:
        job = WORK / jread(active)["id"]
        status = jread(job / "status.json")
        if status["state"] == "running":
            stage(job, "Interrompido", "O serviço reiniciou durante o trabalho; inicie novamente.")
            status = jread(job / "status.json")
            status["state"] = "failed"
            jwrite(job / "status.json", status)
            message = "# Resultado do contrato\n\n- Estado: incompleto\n- Motivo: serviço reiniciado durante a execução.\n"
            save(ROOT / "end_smart_result.md", message)
            save(job / "end_smart_result.md", message)
    except (OSError, ValueError, KeyError):
        pass

def main():
    os.umask(0o077)
    WORK.mkdir(parents=True, exist_ok=True, mode=0o700)
    if len(sys.argv) == 2 and sys.argv[1] == "serve":
        recover_active()
        ThreadingHTTPServer(("127.0.0.1", 8765), Handler).serve_forever()
    elif len(sys.argv) == 3 and sys.argv[1] == "worker" and re.fullmatch(r"[0-9a-f]{16}", sys.argv[2]):
        worker(sys.argv[2])
    else:
        raise SystemExit("usage: site_jobs.py serve|worker JOB_ID")

if __name__ == "__main__":
    main()
