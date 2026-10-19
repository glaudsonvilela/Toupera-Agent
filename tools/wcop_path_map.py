#!/usr/bin/env python3
"""Link local runtime positions to the matching compiler source map."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

from wcop_scope import SOLC, compare_runtime

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output/site-jobs"


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    path.chmod(0o600)


def compiler_map(job):
    output = read(job / "compiler-output.json")
    path = "src/LatamStable.sol"
    name = "LatamStable"
    try:
        deployed = output["contracts"][path][name]["evm"]["deployedBytecode"]
        if deployed.get("sourceMap") and "generatedSources" in deployed:
            return output
    except KeyError:
        pass
    input_data = read(job / "compiler-input.json")
    wanted = input_data["settings"]["outputSelection"][path][name]
    for output_name in ("evm.deployedBytecode.sourceMap", "evm.deployedBytecode.generatedSources"):
        if output_name not in wanted:
            wanted.append(output_name)
    proc = subprocess.run([SOLC, "--standard-json"], input=json.dumps(input_data),
                          capture_output=True, text=True, timeout=120)
    start = proc.stdout.find("{")
    if start < 0:
        raise ValueError("mapa do compilador ausente")
    output = json.loads(proc.stdout[start:])
    if any(e.get("severity") == "error" for e in output.get("errors", [])):
        raise ValueError("compilação do mapa incompleta")
    fresh = output["contracts"][path][name]["evm"]["deployedBytecode"]
    original = read(job / "compiler-output.json")["contracts"][path][name]["evm"]["deployedBytecode"]
    if fresh["object"] != original["object"] or fresh.get("immutableReferences") != original.get("immutableReferences"):
        raise ValueError("saída do mapa difere da compilação conferida")
    return output


def positions(code):
    raw = bytes.fromhex(code.removeprefix("0x"))
    pcs = []
    pc = 0
    while pc < len(raw):
        pcs.append(pc)
        op = raw[pc]
        pc += 1 + (op - 0x5f if 0x60 <= op <= 0x7f else 0)
    return pcs


def expand_map(compressed):
    last = [0, 0, -1, "", 0]
    rows = []
    for entry in compressed.split(";"):
        for index, value in enumerate(entry.split(":")):
            if value != "":
                last[index] = int(value) if index in (0, 1, 2, 4) else value
        rows.append(tuple(last))
    return rows


def line_for(content, offset):
    prefix = content.encode("utf-8")[:offset]
    return prefix.count(b"\n") + 1


def main(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    engine = read(job / "engine-impl.json")
    base = read(job / "trace.json")
    extra = read(job / "adaptive-trace.json") if (job / "adaptive-trace.json").is_file() else {"pcs": [], "observed_transitions": []}
    if base["engine_code_sha256"] != extra.get("engine_code_sha256", base["engine_code_sha256"]):
        raise ValueError("caminhos de códigos diferentes")
    code = bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))
    code_hash = hashlib.sha256(code).hexdigest()
    if code_hash != base["engine_code_sha256"]:
        raise ValueError("caminhos e código diferentes")
    output = compiler_map(job)
    deployed = output["contracts"]["src/LatamStable.sol"]["LatamStable"]["evm"]["deployedBytecode"]
    chain = (job / "implementation.hex").read_text().strip()
    comparison = compare_runtime(deployed["object"], chain, deployed.get("immutableReferences") or {})
    if comparison["state"] not in ("idêntico", "confere sem campos imutáveis"):
        raise ValueError("fonte não ligada ao código do bloco")
    pcs = positions(deployed["object"])
    maps = expand_map(deployed["sourceMap"])
    if len(pcs) < len(maps):
        raise ValueError("mapa tem mais entradas que o código")
    mapped_end_pc = pcs[len(maps) - 1]
    ids = {v["id"]: k for k, v in output["sources"].items()}
    contents = {k: v["content"] for k, v in read(job / "compiler-input.json")["sources"].items()}
    generated = set()
    for item in deployed.get("generatedSources") or []:
        source_id, name, content = item.get("id"), item.get("name"), item.get("contents")
        if (not isinstance(source_id, int) or source_id in ids or
                not isinstance(name, str) or not name or not isinstance(content, str) or
                len(content.encode("utf-8")) > 2_000_000):
            raise ValueError("arquivo interno do compilador inválido")
        label = "(compilador)/" + name
        if label in contents:
            raise ValueError("nome interno repetido")
        ids[source_id] = label
        contents[label] = content
        generated.add(label)
    by_pc = {}
    for pc, (start, length, file_id, _jump, _modifier) in zip(pcs, maps):
        if pc >= len(code):
            continue
        file = ids.get(file_id)
        if file is None or start < 0 or length <= 0 or start + length > len(contents[file].encode("utf-8")):
            by_pc[pc] = None
        else:
            by_pc[pc] = {"file": file, "line": line_for(contents[file], start), "start": start,
                         "length": length, "kind": "generated" if file in generated else "source"}
    engine_pcs = {item["pc"] for item in engine["instructions"]}
    if not set(by_pc).issubset(engine_pcs) or not engine_pcs.issubset(set(pcs)):
        raise ValueError("instruções do motor e mapa do compilador diferentes")
    for pc in engine_pcs - set(by_pc):
        by_pc[pc] = None
    seen = set(base["pcs"]) | set(extra["pcs"])
    if not seen.issubset(engine_pcs):
        raise ValueError("posição observada fora do código")
    counts = {}
    for pc in sorted(engine_pcs):
        where = by_pc[pc]
        key = where["file"] if where else "(sem local de fonte)"
        item = counts.setdefault(key, {"total_pcs": 0, "seen_pcs": 0, "seen_lines": set()})
        item["total_pcs"] += 1
        if pc in seen:
            item["seen_pcs"] += 1
            if where:
                item["seen_lines"].add(where["line"])
    for item in counts.values():
        item["seen_lines"] = sorted(item["seen_lines"])
        item["seen_line_count"] = len(item["seen_lines"])
    static = {(row["from"], row["to"]) for row in engine["control_edges"]}
    transitions = set(tuple(x) for x in base["observed_transitions"])
    transitions |= set(tuple(x) for x in extra["observed_transitions"])
    def loc(node):
        try:
            pc = int(node.removeprefix("pc:"))
        except ValueError:
            return None
        return by_pc.get(pc)
    open_links = []
    for left, right in sorted(transitions - static):
        open_links.append({"from": left, "to": right, "from_source": loc(left), "to_source": loc(right)})
    result = {
        "format": 1, "block": base["block"], "contract": base["contract"],
        "implementation": base["implementation"], "engine_code_sha256": code_hash,
        "source_relation": comparison["state"], "compiler_input_sha256": hashlib.sha256(
            (job / "compiler-input.json").read_bytes()).hexdigest(),
        "all_pcs": len(engine_pcs), "observed_pcs": len(seen),
        "last_compiler_mapped_pc": mapped_end_pc,
        "positions_after_compiler_map": sum(pc > mapped_end_pc for pc in engine_pcs),
        "observed_positions_after_compiler_map": sum(pc > mapped_end_pc for pc in seen),
        "source_assigned_pcs": sum(v["total_pcs"] for k, v in counts.items() if k != "(sem local de fonte)"),
        "observed_source_assigned_pcs": sum(v["seen_pcs"] for k, v in counts.items() if k != "(sem local de fonte)"),
        "generated_assigned_pcs": sum(v["total_pcs"] for k, v in counts.items() if k in generated),
        "observed_generated_assigned_pcs": sum(v["seen_pcs"] for k, v in counts.items() if k in generated),
        "files": counts,
        "pc_locations": [{"pc": pc, **where} for pc, where in sorted(by_pc.items()) if where],
        "observed_links_outside_static_map": len(open_links),
        "links_to_review": open_links,
        "limits": ["Uma localização do compilador não prova que o ramo seja alcançável.",
                   "Campos imutáveis mascarados mantêm a ressalva do vínculo.",
                   "Linhas observadas no runtime e ramos medidos pela fonte são contagens distintas."]
    }
    write(job / "path-map.json", result)
    return result


if __name__ == "__main__":
    result = main(sys.argv[1])
    print(json.dumps({"observed_pcs": result["observed_pcs"],
                      "source_assigned_pcs": result["source_assigned_pcs"],
                      "observed_links_outside_static_map": result["observed_links_outside_static_map"]}))
