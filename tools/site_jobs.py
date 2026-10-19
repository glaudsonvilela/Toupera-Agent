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

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output" / "site-jobs"
ADDRESS = "0x8a1d45e102e886510e891d2ec656a708991e2d76"
RPC = "https://ethereum.publicnode.com"
CHAIN = 1
ENGINE = ROOT / "target/release/security-engine"
SLOT = "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"

def save(path, content):
    path = Path(path)
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
    with urllib.request.urlopen(req, timeout=30) as response:
        data = response.read(10_000_001)
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

def command(args, path, timeout=90, cwd=None):
    env = dict(os.environ)
    env["PATH"] = "/home/toupera/.local/bin:/home/toupera/.foundry/bin:/usr/local/bin:/usr/bin:/bin"
    with open(path, "w", encoding="utf-8") as out:
        try:
            result = subprocess.run(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
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

def run_tools(job, source, runtime_path, block_number):
    rows = []
    logs = job / "tools"
    logs.mkdir(mode=0o700)
    main = prepare_source(job, source)
    if main:
        stage(job, "Slither", "Compilando a fonte verificada e reunindo sinais.")
        output = job / "slither.json"
        code = command(["/home/toupera/.local/bin/slither", str(main), "--solc", "/usr/local/bin/solc",
                        "--solc-args", "--base-path .", "--json", str(output)],
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
                      "; compilador local 0.8.37; saída " + str(code))
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
    stage(job, "Foundry", "Executando três regras básicas em cópia local do bloco.")
    foundry = job / "foundry"
    (foundry / "test").mkdir(parents=True, mode=0o700)
    shutil.copyfile(ROOT / "tools/wcop_readonly.t.sol", foundry / "test/Wcop.t.sol")
    save(foundry / "foundry.toml", '[profile.default]\nsolc = "/usr/local/bin/solc"\ntest = "test"\noptimizer = true\n')
    forge_log = logs / "foundry.log"
    forge_code = command(["/home/toupera/.foundry/bin/forge", "test", "--root", str(foundry),
                          "--fork-url", RPC, "--fork-block-number", str(block_number), "-vv"],
                         forge_log, 120)
    forge_text = forge_log.read_text(encoding="utf-8", errors="replace")
    passed = re.search(r"(\d+) tests passed, (\d+) failed", forge_text)
    if forge_code == 0 and passed and passed.group(1) == "3" and passed.group(2) == "0":
        rows.append(tool_line("Foundry", "executado", "3 regras locais passaram no bloco " + str(block_number) +
                              ": aprovação, emissão não autorizada e pausa não autorizada; isso não cobre outros caminhos"))
    else:
        rows.append(tool_line("Foundry", "limite" if forge_code == 124 else "falhou",
                              "regras locais não concluídas; saída " + str(forge_code)))
    rows.extend([
        tool_line("Echidna", "pendente", "exige propriedade e harness de sequência"),
        tool_line("Medusa", "pendente", "exige propriedade e harness de sequência"),
        tool_line("Certora", "indisponível", "CLI sem chave; serviço externo não acionado"),
        tool_line("Halmos", "pendente", "exige propriedade Foundry curta"),
        tool_line("Kontrol", "pendente", "exige propriedade formal e premissas"),
    ])
    jwrite(job / "tools.json", rows)
    return rows

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
        save(ROOT / "result_smart.md", result)
        save(job / "result_smart.md", result)
        stage(job, "Ferramentas", "Executando leituras compatíveis e registrando as demais.")
        selected_source = impl_source if impl_source and impl_source.get("source") else source
        selected_runtime = impl_runtime_path or runtime_path
        rows = run_tools(job, selected_source, selected_runtime, block_number)
        final = result.replace("# Resultado do motor Rust", "# Resultado do contrato", 1)
        final += "\n## Ferramentas\n\n"
        for item in rows:
            final += "- " + item["name"] + " — " + item["state"] + ": " + item["detail"] + ".\n"
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
                    (ROOT / old).unlink(missing_ok=True)
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
