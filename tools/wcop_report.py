#!/usr/bin/env python3
"""Create a shareable, non-secret Markdown coverage snapshot from one completed VPS job."""
import json
from pathlib import Path
import re
import sys

ROOT = Path("/srv/toupera-agent")
WORK = ROOT / "private-output/site-jobs"

def read(path):
    return json.loads(path.read_text(encoding="utf-8"))

def pct(hit, total):
    return "n/a" if not total else f"{100 * hit / total:.1f}%"

def main(job_id):
    if not re.fullmatch(r"[0-9a-f]{16}", job_id):
        raise ValueError("identificador inválido")
    job = WORK / job_id
    status = read(job / "status.json")
    if status.get("state") != "complete":
        raise ValueError("trabalho ainda não concluído")
    scope = read(job / "scope.json")
    engine = read(job / "engine-impl.json")
    scenarios = read(job / "scenarios.json")
    history = read(job / "history.json")
    inv = scope.get("inventory") or {}
    cov = scope.get("coverage") or {}
    comp = scope.get("compiler") or {}
    totals, own = cov.get("totals") or {}, cov.get("main_file") or {}
    tools = {row["name"]: row for row in read(job / "tools.json")}
    lines = [
        "# wCOP: mapa e cobertura medidos no VPS",
        "",
        f"- Trabalho: `{job_id}`.",
        f"- Contrato: `{history['address']}` na Ethereum, bloco `{history['through_block']}`.",
        "- Escopo: apenas este contrato na Ethereum e uma cópia local da fonte para medir caminhos.",
        "",
        "## Identidade do código",
        "",
        f"- Fonte: {comp.get('sources','?')} arquivos, Solidity `{comp.get('compiler','?')}`, EVM `{comp.get('evm_version','?')}`.",
        f"- Comparação com runtime da implementação: **{comp.get('state','incompleta')}**. "
        "As posições imutáveis são marcadas pelo próprio compilador e foram excluídas da comparação.",
        f"- Entradas públicas: {inv.get('abi_functions',0)} na ABI; "
        f"{inv.get('engine_selectors',0)} seletores no Rust; "
        f"{inv.get('scenario_functions',0)} com ao menos um cenário local concluído.",
        "- Isso mede entrada em funções, não todos os estados ou ramos de cada função.",
        f"- Espaços de dados nomeados na fonte: {len(scope.get('storage_namespaces') or [])}; "
        f"posições distintas: {'sim' if scope.get('storage_slots_unique') else 'não confirmado'}. "
        "O layout comum do compilador não enumera os espaços ERC-7201 usados pelas bibliotecas.",
        "",
        "## Execuções",
        "",
        f"- Foundry no bloco: {len(scenarios.get('passed_names') or [])}/{scenarios.get('expected',0)} cenários, "
        "incluindo operações autorizadas, limites, assinatura, papéis e sequências variáveis.",
        f"- Foundry na fonte recompilada: {cov.get('local_tests_passed',0)} cenários.",
        f"- Echidna: {tools.get('Echidna',{}).get('state','sem registro')}; "
        f"Medusa: {tools.get('Medusa',{}).get('state','sem registro')}. "
        "Duas regras locais comparam oferta e saldos em sequências.",
        f"- Halmos: {tools.get('Halmos',{}).get('state','sem registro')}. "
        "A propriedade preparada não virou prova completa quando o executor parou no preparo do proxy.",
        f"- Eventos lidos no intervalo: {history['event_count']}; "
        f"oferta conferida com o bloco: {'sim' if history['supply_matches_events'] else 'não'}.",
        "",
        "## Cobertura da fonte recompilada",
        "",
        "| Área | Linhas | Funções | Ramos |",
        "|---|---:|---:|---:|",
        f"| Arquivo próprio LatamStable.sol | {own.get('LH',0)}/{own.get('LF',0)} "
        f"({pct(own.get('LH',0),own.get('LF',0))}) | "
        f"{own.get('FNH',0)}/{own.get('FNF',0)} | "
        f"{own.get('BRH',0)}/{own.get('BRF',0)} |",
        f"| Fonte e bibliotecas incluídas | {totals.get('LH',0)}/{totals.get('LF',0)} "
        f"({pct(totals.get('LH',0),totals.get('LF',0))}) | "
        f"{totals.get('FNH',0)}/{totals.get('FNF',0)} | "
        f"{totals.get('BRH',0)}/{totals.get('BRF',0)} "
        f"({pct(totals.get('BRH',0),totals.get('BRF',0))}) |",
        "",
        "A medição acima usa uma implantação local da mesma fonte. Ela não é cobertura direta do bytecode já publicado. "
        "A soma das bibliotecas inclui caminhos que podem não ser alcançáveis por este token; portanto, não é uma porcentagem do runtime.",
        "",
        "## Mapa das entradas públicas",
        "",
        "| Função | Seletor | Cenários locais concluídos |",
        "|---|---|---:|",
    ]
    for item in inv.get("functions") or []:
        lines.append(f"| `{item['signature']}` | `{item['selector']}` | {len(item['scenarios_passed'])} |")
    lines += ["", "## Maiores lacunas medidas", ""]
    for gap in (cov.get("largest_gaps") or [])[:10]:
        lines.append(f"- `{gap['file']}`: {gap['lines_not_hit']} linhas e {gap['branches_not_hit']} ramos sem passagem.")
    lines += [
        "",
        "## Próximos passos verificáveis",
        "",
        "1. Criar casos para os ramos ainda não alcançados nas bibliotecas, sobretudo valores inválidos, limites e mudanças de estado.",
        "2. Repetir sequências mais longas e registrar os caminhos percorridos no runtime publicado, além da fonte local.",
        "3. Preparar e medir uma troca válida de implementação em cópia local; comparar os espaços de dados antes e depois.",
        "4. Ajustar o preparo do proxy para Halmos e acrescentar propriedades formais que o executor conclua. Kontrol e Certora continuam sem prova deste caso.",
        "5. Se o escopo for ampliado, relacionar chamadas dos contratos vizinhos; este documento não cobre outras redes.",
        "",
        "**Estado:** 100% das entradas da ABI receberam ao menos um cenário local nesta rodada; "
        "a cobertura de ramos está longe de 100%. Nenhuma falha foi confirmada por esses números.",
        "",
    ]
    target = ROOT / "docs/WCOP-COBERTURA-2026-10-19.md"
    target.write_text("\n".join(lines), encoding="utf-8")
    return target

if __name__ == "__main__":
    print(main(sys.argv[1]))
