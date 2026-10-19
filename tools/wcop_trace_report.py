#!/usr/bin/env python3
"""Shareable summary of a completed, identity-bound local runtime trace."""
import json
from pathlib import Path
import re
import sys

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output/site-jobs"


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def main(job_id):
    if not re.fullmatch(r"[0-9a-f]{16}", job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    status = read(job / "status.json")
    if status.get("state") != "complete":
        raise ValueError("trabalho ainda não concluído")
    raw = read(job / "trace.json")
    linked = read(job / "trace-reconcile.json")
    source = read(job / "coverage.json")
    if raw["block"] != linked["block"] or raw["contract"] != linked["contract"]:
        raise ValueError("leituras em blocos diferentes")
    calls = raw["cases"]
    sequence_calls = sum(item.get("group") == "sequência" for item in calls)
    local_commits = sum(bool(item.get("state_committed_locally")) for item in calls)
    mismatches = sum(item.get("expected_outcome_matches") is False for item in calls)
    lines = [
        "# wCOP: caminhos observados no runtime publicado",
        "",
        f"- Trabalho: `{job_id}`; Ethereum, bloco `{linked["block"]}`.",
        f"- Endereço: `{linked["contract"]}`.",
        "- Método: chamadas isoladas e uma sequência com estado sobre uma cópia local do bloco; o bytecode vem da implementação publicada e o motor Rust confere sua identidade antes de ligar PCs ao mapa.",
        "- Nenhuma chamada deste ensaio foi enviada como transação à rede pública.",
        "",
        "## Medição",
        "",
        "| Item | Observado | Total do mapa |",
        "|---|---:|---:|",
        f"| PCs de instruções | {linked["observed_instruction_pcs"]} | {linked["disassembled_instruction_pcs"]} |",
        f"| Blocos | {linked["observed_blocks"]} | {linked["engine_blocks"]} |",
        f"| Ligações entre blocos presentes no mapa | {linked["matched_static_edges"]} | {linked["observed_transitions"]} transições observadas |",
        "",
        f"- Chamadas: {linked["calls"]}; sequência: {sequence_calls}; passos gravados só na cópia local: {local_commits}; divergências de resultado esperado: {mismatches}; revertidas conforme esperado ou por condição local: {linked["reverted_calls"]}; erros da leitura: {linked["trace_errors"]}.",
        f"- Transições observadas ainda sem ligação no mapa estático: {linked["unmatched_transitions"]}. Isso aponta trabalho no mapa, não uma falha do contrato.",
        f"- Dúvidas do motor em PCs visitados: {linked["observed_uncertainties"]}. A lista ordenada fica no JSON privado; a presença no caminho não confirma problema.",
        f"- Medição separada da fonte recompilada: {source["totals"]["BRH"]}/{source["totals"]["BRF"]} ramos no conjunto fonte e bibliotecas; veja [a segunda medição](WCOP-COBERTURA-ETAPA2-2026-10-19.md).",
        "",
        "## Chamadas locais",
        "",
        "| Caso | Grupo | Seletor | Estado local | PCs distintos |",
        "|---|---|---|---|---:|",
    ]
    for item in calls:
        state = "erro" if item.get("error") else ("reverteu" if item.get("failed") else "concluiu")
        lines.append(f"| {item["case"]} | {item.get("group","isolada")} | `{item.get("selector", "")}` | {state} | {item.get("known_pcs",0)} |")
    lines.extend([
        "",
        "## Limites e próximos passos",
        "",
        "- As chamadas isoladas não guardam estado entre si; a sequência local usa um conjunto limitado de operações e papéis. Outras combinações podem percorrer outros PCs.",
        "- O total de instruções desmontadas não é o total comprovado de instruções alcançáveis. Portanto, a razão observada não é porcentagem de cobertura completa do contrato.",
        "- Próxima etapa: ampliar sequências com estado na cópia local e ligar as transições ainda ausentes ao mapa Rust, com verificação de identidade em cada rodada.",
        "- Nenhuma falha do contrato foi confirmada por esta medição.",
        "",
    ])
    target = ROOT / "docs/WCOP-CAMINHOS-RUNTIME-2026-10-19.md"
    target.write_text("\n".join(lines), encoding="utf-8")
    return target


if __name__ == "__main__":
    print(main(sys.argv[1]))
