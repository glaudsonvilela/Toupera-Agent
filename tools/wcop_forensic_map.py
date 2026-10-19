#!/usr/bin/env python3
"""Offline reconciliation of one pinned wCOP job's runtime map and source coverage."""
import collections
import hashlib
import json
import os
from pathlib import Path
import sys

from wcop_path_map import expand_map, positions

ROOT = Path("/srv/toupera-agent/private-output/site-jobs")


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalise(path):
    for prefix, replacement in (
        ("lib/openzeppelin-contracts-upgradeable/contracts/", "@openzeppelin/contracts-upgradeable/"),
        ("lib/openzeppelin-contracts/contracts/", "@openzeppelin/contracts/"),
    ):
        if prefix in path:
            return replacement + path.split(prefix, 1)[1]
    return path


def lcov_branches(path):
    branches = collections.defaultdict(list)
    declared = collections.Counter()
    current = None
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            current = line[3:]
        elif line == "end_of_record":
            current = None
        elif current is not None and not current.startswith("test/"):
            if line.startswith("BRDA:"):
                line_no, block, branch, taken = line[5:].split(",")
                branches[(current, int(line_no))].append((block, branch, taken))
            elif line.startswith("BRF:"):
                declared["total"] += int(line[4:])
            elif line.startswith("BRH:"):
                declared["hit"] += int(line[4:])
    if sum(map(len, branches.values())) != declared["total"]:
        raise ValueError("LCOV branch records and totals differ")
    return branches, dict(declared)


def atomic_private_write(path, value):
    payload = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()
    temp = path.with_name("." + path.name + "." + str(os.getpid()) + ".tmp")
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "wb") as out:
            out.write(payload)
            out.flush()
            os.fsync(out.fileno())
        os.replace(temp, path)
        os.chmod(path, 0o600)
        parent = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
    finally:
        if temp.exists():
            temp.unlink()


def analyse(job_id):
    if len(job_id) != 16 or any(c not in "0123456789abcdef" for c in job_id):
        raise ValueError("invalid job id")
    job = ROOT / job_id
    names = ("engine-impl.json", "path-map.json", "trace.json", "adaptive-trace.json",
             "compiler-input.json", "compiler-output.json", "coverage/coverage.info")
    digests = {name: sha256(job / name) for name in names}
    engine, line_map, base, extra, compiler_input, compiler_output = (
        read(job / name) for name in names[:6]
    )
    raw = bytes.fromhex(engine["normalized_hex"].removeprefix("0x"))
    code_hash = hashlib.sha256(raw).hexdigest()
    if any(item["engine_code_sha256"] != code_hash for item in (line_map, base, extra)):
        raise ValueError("runtime identity mismatch")
    if (line_map["block"], line_map["contract"]) != (base["block"], base["contract"]) or (
        extra["block"], extra["contract"]) != (base["block"], base["contract"]):
        raise ValueError("block or contract mismatch")
    source_path = "src/LatamStable.sol"
    deployed = compiler_output["contracts"][source_path]["LatamStable"]["evm"]["deployedBytecode"]
    compiler_pcs = positions(deployed["object"])
    index = {pc: i for i, pc in enumerate(compiler_pcs)}
    source_rows = expand_map(deployed["sourceMap"])
    if len(source_rows) > len(compiler_pcs):
        raise ValueError("source map exceeds bytecode")
    source_ids = {v["id"]: name for name, v in compiler_output["sources"].items()}
    for generated in deployed.get("generatedSources") or []:
        if generated["id"] in source_ids:
            raise ValueError("duplicate source id")
        source_ids[generated["id"]] = "(compilador)/" + generated["name"]
    instructions = {item["pc"]: item for item in engine["instructions"]}
    blocks = {item["id"]: item for item in engine["blocks"]}
    seen = set(base["pcs"]) | set(extra["pcs"])
    if not seen.issubset(instructions):
        raise ValueError("observed PC outside runtime")
    locations = {item["pc"]: item for item in line_map["pc_locations"]}
    reasons = collections.Counter()
    for pc in seen:
        row_index = index.get(pc)
        if row_index is None:
            reason = "outside_compiler_instructions"
        elif row_index >= len(source_rows):
            reason = "after_compiler_source_map"
        else:
            start, length, source_id, *_ = source_rows[row_index]
            reason = "compiler_file_id_unassigned" if source_id == -1 else (
                "other_unassigned_source_id" if source_id not in source_ids else "assigned")
        if reason == "assigned" and pc not in locations:
            raise ValueError("assigned PC missing from saved line map")
        if reason != "assigned" and pc in locations:
            raise ValueError("unassigned PC has a saved source location")
        reasons[reason] += 1
    if reasons["assigned"] != line_map["observed_source_assigned_pcs"] or len(seen) != line_map["observed_pcs"]:
        raise ValueError("observed map counts differ")
    static = {(edge["from"], edge["to"]) for edge in engine["control_edges"]}
    observed = {tuple(edge) for item in (base, extra) for edge in item["observed_transitions"]}
    open_links = observed - static
    saved_links = {(item["from"], item["to"]) for item in line_map["links_to_review"]}
    if open_links != saved_links or len(open_links) != line_map["observed_links_outside_static_map"]:
        raise ValueError("open link list differs from traces")
    link_kinds = collections.Counter()
    source_files = collections.Counter()
    from_opcodes = collections.Counter()
    terminal_opcodes = collections.Counter()
    terminal_jump_markers = collections.Counter()
    link_lines = set()
    for link in line_map["links_to_review"]:
        endpoints = (link["from_source"], link["to_source"])
        link_kinds[tuple("none" if item is None else item["kind"] for item in endpoints)] += 1
        source_files[(endpoints[0] or {}).get("file", "none")] += 1
        from_pc = int(link["from"].removeprefix("pc:"))
        from_opcodes[instructions[from_pc]["mnemonic"]] += 1
        terminal_pc = blocks[link["from"]]["instruction_pcs"][-1]
        terminal_opcodes[instructions[terminal_pc]["mnemonic"]] += 1
        terminal_index = index.get(terminal_pc)
        marker = (source_rows[terminal_index][3] if terminal_index is not None
                  and terminal_index < len(source_rows) else "outside")
        terminal_jump_markers[marker] += 1
        for item in endpoints:
            if item and item["kind"] == "source":
                link_lines.add((normalise(item["file"]), item["line"]))
    source_matches = 0
    for name, source in compiler_input["sources"].items():
        coverage_source = job / "coverage" / normalise(name)
        if not coverage_source.is_file() or coverage_source.read_text(encoding="utf-8") != source["content"]:
            raise ValueError("coverage source differs from compiler input: " + name)
        source_matches += 1
    branches, declared = lcov_branches(job / "coverage/coverage.info")
    coverage = read(job / "coverage.json")
    if declared != {"total": coverage["totals"]["BRF"], "hit": coverage["totals"]["BRH"]}:
        raise ValueError("coverage summary differs from LCOV")
    seen_source_lines = {(normalise(locations[pc]["file"]), locations[pc]["line"])
                         for pc in seen if pc in locations and locations[pc]["kind"] == "source"}
    conditional_edges = 0
    conditional_edges_outside_static = 0
    conditional_edges_on_lcov_line = 0
    observed_terminal_opcodes = collections.Counter()
    for origin, destination in observed:
        terminal_pc = blocks[origin]["instruction_pcs"][-1]
        opcode = instructions[terminal_pc]["mnemonic"]
        observed_terminal_opcodes[opcode] += 1
        if opcode == "JUMPI":
            conditional_edges += 1
            conditional_edges_outside_static += (origin, destination) not in static
            loc = locations.get(terminal_pc)
            if loc and loc["kind"] == "source" and (normalise(loc["file"]), loc["line"]) in branches:
                conditional_edges_on_lcov_line += 1
    links_on_branch_lines = sum(any(item and item["kind"] == "source" and
        (normalise(item["file"]), item["line"]) in branches
        for item in (link["from_source"], link["to_source"]))
        for link in line_map["links_to_review"])
    return {
        "format": 1, "job_id": job_id, "contract": base["contract"], "block": base["block"],
        "engine_code_sha256": code_hash, "source_relation": line_map["source_relation"],
        "compiler_map_boundary": {"last_pc": line_map["last_compiler_mapped_pc"],
            "positions_after_map": line_map["positions_after_compiler_map"],
            "observed_after_map": line_map["observed_positions_after_compiler_map"]},
        "input_sha256": digests,
        "pc_classification": {"observed": len(seen), "reasons": dict(sorted(reasons.items()))},
        "open_links": {"count": len(open_links),
            "endpoint_kinds": {"->".join(key): value for key, value in sorted(link_kinds.items())},
            "unique_origins": len({a for a, _ in open_links}),
            "unique_destinations": len({b for _, b in open_links}),
            "origin_block_entry_opcode_counts": dict(sorted(from_opcodes.items())),
            "terminal_opcode_counts": dict(sorted(terminal_opcodes.items())),
            "terminal_jump_marker_counts": dict(sorted(terminal_jump_markers.items())),
            "top_origin_files": source_files.most_common(8)},
        "source_branch_correspondence": {"identical_source_files": source_matches,
            "lcov_branch_records": declared["total"], "lcov_hit_branch_records": declared["hit"],
            "lcov_branch_lines": len(branches),
            "branch_lines_with_observed_runtime_pc": len(seen_source_lines & set(branches)),
            "branch_lines_at_open_link_endpoints": len(link_lines & set(branches)),
            "open_links_touching_lcov_branch_line": links_on_branch_lines,
            "observed_edge_terminal_opcode_counts": dict(sorted(observed_terminal_opcodes.items())),
            "observed_conditional_edges": conditional_edges,
            "conditional_edges_outside_static_map": conditional_edges_outside_static,
            "conditional_edges_with_lcov_origin_line": conditional_edges_on_lcov_line},
        "limits": ["The -1 compiler source ID supplies no defensible source line.",
            "An observed edge absent from the static graph is not a vulnerability by itself.",
            "Trace edges use block entry IDs; the terminal opcode and compiler jump marker classify the block, not the per-case stack state or branch taken.",
            "LCOV was measured on recompiled source; line overlap does not prove runtime branch coverage."],
    }


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: wcop_forensic_map.py <job-id>")
    report = analyse(sys.argv[1])
    target = ROOT / sys.argv[1] / "forensic-map.json"
    atomic_private_write(target, report)
    print(json.dumps({"job_id": report["job_id"], "pc_classification": report["pc_classification"],
        "open_links": report["open_links"],
        "source_branch_correspondence": report["source_branch_correspondence"]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
