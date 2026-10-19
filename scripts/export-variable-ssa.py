#!/usr/bin/env python3
"""Export local SlithIR SSA reads/writes without sending source anywhere."""

import argparse
import hashlib
import json
from pathlib import Path

from slither.slither import Slither


def operand(value, contract, function):
    if value is None:
        return None
    original = getattr(value, "non_ssa_version", value)
    location = getattr(original, "source_mapping", None)
    name = getattr(original, "name", None)
    return {
        "key": f"{contract}:{function}:{type(value).__name__}:{value}",
        "name": name if isinstance(name, str) else None,
        "start": getattr(location, "start", None),
        "length": getattr(location, "length", None),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("solc", type=Path)
    parser.add_argument("--evm-version", default="cancun")
    args = parser.parse_args()
    source = args.source.resolve()
    compiled = Slither(
        str(source),
        solc=str(args.solc.resolve()),
        solc_args=f"--evm-version {args.evm_version}",
    )
    operations = []
    for contract in compiled.contracts:
        for function in contract.functions_and_modifiers_declared:
            for node in function.nodes:
                location = getattr(node, "source_mapping", None)
                for ir in node.irs_ssa:
                    written = operand(getattr(ir, "lvalue", None), contract.name, function.full_name)
                    if written is None:
                        continue
                    reads = [
                        item
                        for value in getattr(ir, "read", [])
                        if (item := operand(value, contract.name, function.full_name)) is not None
                    ]
                    if not reads:
                        continue
                    operations.append(
                        {
                            "contract": contract.name,
                            "function": function.full_name,
                            "operator": type(ir).__name__,
                            "node_start": getattr(location, "start", None),
                            "write": written,
                            "reads": reads,
                        }
                    )
    report = {
        "schema": "slither-ssa-v1",
        "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "source_name": source.name,
        "operation_count": len(operations),
        "operations": operations,
        "limitations": [
            "SSA links are local to each Slither function and do not prove path feasibility",
            "External effects and cross-transaction state are not reconstructed here",
        ],
    }
    json.dump(report, __import__("sys").stdout, separators=(",", ":"))
    print()


if __name__ == "__main__":
    main()
