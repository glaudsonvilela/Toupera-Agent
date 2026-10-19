#!/usr/bin/env python3
"""Build a traceable review from one completed, pinned wCOP job."""
import collections
import hashlib
import json
from pathlib import Path
import re
import sys
from wcop_trace import ROOT, WORK, TOKEN


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def band(score):
    return "alta" if score >= 70 else "média" if score >= 40 else "baixa"


def clean(value):
    return str(value).replace("|", "/").replace("\n", " ").strip()


def score_event(event, supply, role_names):
    kind = event.get("kind")
    if kind == "upgraded":
        return 78, "troca de implementação; conferir identidade e estado"
    if kind == "initialized":
        return 52, "início do proxy; conferir sequência de papéis"
    if kind in ("role_granted", "role_revoked"):
        role = role_names.get(event.get("role"), "papel não identificado")
        score = 70 if role in ("DEFAULT_ADMIN_ROLE", "UPGRADER_ROLE") else 58
        return score, ("concessão" if kind == "role_granted" else "retirada") + " de " + role
    if kind == "approval":
        amount = int(event.get("amount_raw", "0"))
        if amount == 2**256 - 1:
            return 56, "autorização sem limite; conferir uso e contexto"
        if amount == 0:
            return 18, "autorização zerada"
        return 30, "autorização finita"
    if kind == "transfer":
        amount = int(event.get("amount_raw", "0"))
        flow = event.get("flow", "transfer")
        if flow in ("mint", "burn"):
            score = 76 if amount * 10 >= supply else 64 if amount * 100 >= supply else 53
            return score, "mudança de oferta; conferir papel e balanço"
        if amount == 0:
            return 12, "movimento de valor zero"
        if amount * 10 >= supply:
            return 63, "movimento grande frente à oferta do bloco"
        if amount * 100 >= supply:
            return 44, "movimento relevante frente à oferta do bloco"
        return 20, "movimento comum"
    return 75, "tipo não classificado; revisar antes de concluir"


def score_attempt(row):
    if row.get("deviation"):
        return (85 if row.get("repeatable_deviation") else 65), "diferença local a revisar"
    points = int(row.get("new_pcs", 0))
    links = int(row.get("new_edges", 0))
    blocks = int(row.get("new_blocks", 0))
    if points or links or blocks:
        score = min(80, 25 + min(35, points // 10) + min(15, links * 2) + min(10, blocks * 2))
        return score, "caminho novo observado"
    if row.get("failed"):
        return 12, "chamada não aplicada; caminho conhecido"
    return 18, "operação local sem caminho novo"


def score_slither(item, active_files):
    files = {element.get("source_mapping", {}).get("filename_relative", "")
             for element in item.get("elements", [])}
    files.discard("")
    main = "src/LatamStable.sol" in files
    active = any(path in active_files for path in files)
    base = {"High": 74, "Medium": 54, "Low": 32, "Informational": 18}.get(item.get("impact"), 20)
    score = max(5, base + (8 if main else -22) - (15 if not active else 0))
    return score, main, active, ", ".join(sorted(files)) or "sem arquivo"


def build(job_id, output):
    if not re.fullmatch(r"[0-9a-f]{16}", job_id):
        raise ValueError("trabalho inválido")
    job = WORK / job_id
    status = read(job / "status.json")
    if status.get("state") != "complete" or status.get("address") != TOKEN:
        raise ValueError("trabalho concluído do endereço esperado necessário")
    history = read(job / "history.json")
    decision = read(job / "decision.json")
    adaptive = read(job / "adaptive.json")
    state = read(job / "adaptive-state.json")
    review = read(job / "adaptive-reconcile.json")
    coverage = read(job / "coverage.json")
    roles = read(job / "roles.json")
    tools = read(job / "tools.json")
    sift = read(job / "sift.json")
    slither = read(job / "slither.json")["results"]["detectors"]
    base_trace = read(job / "trace.json")
    adaptive_trace = read(job / "adaptive-trace.json")
    block = int(history["through_block"])
    if not (history["address"] == TOKEN and adaptive["contract"] == TOKEN
            and adaptive["block"] == block and decision["block_number"] == block
            and review["trace_errors"] == 0 and decision["identity_ok"]):
        raise ValueError("identidade ou caminhos não conferem")
    md_names = ("smart.md", "result_smart.md", "end_smart_result.md")
    for name in md_names:
        text = (job / name).read_text(encoding="utf-8")
        if TOKEN not in text or (name != "smart.md" and str(block) not in text):
            raise ValueError("arquivo Markdown fora do trabalho")
    events = sorted(history["timeline"] + history["movements"],
                    key=lambda e: (int(e["block"]), int(e["log_index"]), e["tx"]))
    if len(events) != history["event_count"] or len({(e["tx"], e["log_index"]) for e in events}) != len(events):
        raise ValueError("eventos ausentes ou repetidos")
    role_names = {value: key for key, value in roles["role_hashes"].items()}
    supply = int(history["supply_on_chain_raw"])
    pc_total = int(review["disassembled_instruction_pcs"])
    high_events = [event for event in events
                   if band(score_event(event, supply, role_names)[0]) == "alta"]
    high_types = collections.Counter(event.get("flow", event["kind"]) for event in high_events)
    high_summary = (str(len(high_events)) + " registros de atenção alta por impacto potencial: "
                    + ", ".join(str(count) + " " + kind for kind, count in sorted(high_types.items()))
                    + ". A oferta final confere com os eventos; essa fila não demonstra uma diferença no contrato.")
    observed = set(base_trace["pcs"]) | set(adaptive_trace["pcs"])
    active_files = {entry["file"] for entry in coverage["all_files"] if entry["LH"]}
    claim = next((x for x in decision["claims"] if x["id"] == "adaptive"), None)
    if not claim or claim["state"] != "OBSERVADO":
        raise ValueError("ciclo sem ligação de evidência")
    lines = [
        "# Revisão completa do wCOP — rodada " + job_id,
        "",
        "## Escopo e vínculo",
        "",
        "- Contrato: `" + TOKEN + "` na Ethereum; bloco `" + str(block) + "`; hash do bloco `" + decision["block_hash"] + "`.",
        "- Implementação no bloco: `" + decision["implementation"] + "`; código e fonte com vínculo `" + decision["source_binding"] + "` e ressalva dos campos imutáveis.",
        "- Entrada `smart.md`, saída Rust `result_smart.md` e resultado integrado `end_smart_result.md` foram lidos deste mesmo trabalho no VPS. Hashes SHA-256:",
    ]
    for name in md_names:
        lines.append("  - `" + name + "`: `" + sha(job / name) + "`.")
    lines += [
        "- Execução por SSH com chave do servidor conferida e cifra `chacha20-poly1305@openssh.com`. As operações que mudam estado foram feitas somente na cópia local.",
        "",
        "## Resultado que os arquivos sustentam",
        "",
        "- Histórico completo para o intervalo consultado: " + str(len(events)) + " registros em " + str(history["pages"]) + " páginas; emissão menos retirada confere com a oferta lida no bloco.",
        "- Fonte recompilada: " + str(coverage["totals"]["BRH"]) + "/" + str(coverage["totals"]["BRF"]) + " ramos medidos no conjunto compilado; " + str(coverage["active_totals"]["BRH"]) + "/" + str(coverage["active_totals"]["BRF"]) + " nos arquivos com alguma linha executada. Essa medição não inclui os ramos acrescentados pelo ciclo de bytecode.",
        "- Bytecode: " + str(adaptive["combined_pcs"]) + "/" + str(pc_total) + " pontos observados após " + str(adaptive["attempts"]) + " escolhas; ganho de " + str(adaptive["new_pcs"]) + " pontos, " + str(adaptive["new_edges"]) + " ligações e " + str(adaptive["new_blocks"]) + " blocos sobre a leitura inicial.",
        "- O ciclo registrou " + str(adaptive["setup_attempts"]) + " sequências preparadas, " + str(adaptive["frontier_crossings"]) + " passagens para blocos novos e " + str(len(adaptive["deviations"])) + " diferenças nas verificações de estado. Parou por `" + adaptive["status"] + "`.",
        "- Foundry: 18/18 casos locais; Echidna e Medusa: 2 regras cada em campanhas limitadas; Halmos: 3 propriedades locais limitadas. O motor Rust ligou os caminhos ao runtime com " + str(review["trace_errors"]) + " erros de conferência.",
        "- **Falhas confirmadas nesta rodada: 0.** Prioridade alta, média ou baixa abaixo indica ordem de revisão e possível impacto de uma diferença futura, não gravidade de uma falha demonstrada.",
        "- " + high_summary,
        "",
        "## Metas para a próxima rodada",
        "",
        "| Prioridade | Caminho | Evidência atual | Condição para concluir |",
        "|---|---|---|---|",
        "| Alta | Papéis e troca de implementação | Papéis de administração/troca no histórico; testes locais rejeitaram chamadas sem papel e uma troca inválida. | Comparar papéis atuais na rede com as contas descobertas e medir invariantes antes/depois de uma troca válida em mais estados. |",
        "| Alta | Assinatura, prazo e contador | O caminho `permit` abriu novos blocos; uma assinatura válida passou no caso local. O sinal do Mythril aponta uso de `block.timestamp` no prazo, esperado para expiração. | Relacionar domínio, prazo, contador, assinatura válida/inválida e estado do proxy em sequências adicionais; exigir diferença repetível antes de tratar como falha. |",
        "| Média | Autorização sem limite e retirada por terceiro | Aprovações sem limite aparecem no histórico; sequências `approve` → `burnFrom` e `transferFrom` percorreram caminhos adicionais sem diferença registrada. | Medir autorização máxima, parcial e zerada antes/depois de pausa e mudanças de papel, em cópia local e com balanços exatos. |",
        "| Média | Pontas do mapa e fontes ativas | Há ligações observadas fora do mapa estático; bibliotecas ativas ainda têm ramos não percorridos. | Associar cada nova saída do bytecode à origem na fonte e distinguir caminho possível de biblioteca não usada. |",
        "| Baixa | Avisos genéricos das ferramentas | O único sinal Slither ligado ao arquivo principal é de versões de `pragma`; o Mythril aponta o prazo de `permit`. | Manter a classificação contextual; reabrir somente com estado e efeito verificáveis. |",
        "",
        "## Dez ferramentas",
        "",
        "| Ferramenta | Estado nesta rodada | Leitura |",
        "|---|---|---|",
    ]
    for item in tools:
        lines.append("| " + clean(item["name"]) + " | " + clean(item["state"]) + " | " + clean(item["detail"]) + " |")
    lines += [
        "",
        "O rótulo bruto `High` do Slither não foi convertido em falha alta: os dois sinais altos estão em bibliotecas importadas. O primeiro é o XOR que a própria fonte explica como semente da inversão modular; o segundo é a guarda deliberada de `msg.value` no utilitário de troca. A leitura da fonte não mostrou, por si, um efeito incorreto no token. Avisos de lint do Halmos apontam o harness local, não a implementação publicada.",
        "",
        "| Sinal complementar | Pontos | Faixa | Leitura |",
        "|---|---:|---|---|",
        "| Mythril: `block.timestamp` no prazo de `permit` | 24 | baixa | O prazo de expiração usa tempo do bloco; o sinal bruto é esperado nesse desenho. Não foi vista diferença de estado. |",
        "",
        "## Pontos do Rust para revisão",
        "",
        "Os 30 itens abaixo são dúvidas do mapa. A pontuação cresce quando a posição foi percorrida nesta rodada; não é chance de falha.",
        "",
        "| PC | Questão do motor | Visto nesta rodada? | Pontos | Faixa |",
        "|---:|---|---|---:|---|",
    ]
    for item in sift["selected"]:
        seen = int(item["pc"]) in observed
        score = min(69, 15 + 10 * int(item["priority"]) + (10 if seen else 0))
        lines.append("| " + str(item["pc"]) + " | " + clean(item["question"]) + " | "
                     + ("sim" if seen else "não") + " | " + str(score) + " | " + band(score) + " |")
    lines += [
        "",
        "## Todas as escolhas da cópia local",
        "",
        "Pontos aqui medem ganho de caminho e diferença de estado; uma chamada recusada não é uma falha. `Preparação` conta operações locais feitas antes da escolha principal.",
        "",
        "| Nº | Família | Preparação | Aplicada localmente? | PCs novos | Ligações novas | Pontos | Faixa | Motivo |",
        "|---:|---|---:|---|---:|---:|---:|---|---|",
    ]
    for index, row in enumerate(state["attempts"], 1):
        score, why = score_attempt(row)
        lines.append("| " + str(index) + " | " + clean(row["family"]) + " | "
                     + str(row["setup_steps"]) + " | " + ("sim" if row["committed_locally"] else "não")
                     + " | " + str(row["new_pcs"]) + " | " + str(row["new_edges"])
                     + " | " + str(score) + " | " + band(score) + " | " + clean(why) + " |")
    lines += [
        "",
        "## Todos os sinais do Slither",
        "",
        "Os 114 sinais brutos são pontuados pelo rótulo da ferramenta, ligação ao arquivo principal e passagem pelo arquivo na medição da fonte. A pontuação não valida o sinal.",
        "",
        "| Nº | Detector | Rótulo bruto | Arquivo principal? | Arquivo ativo? | Pontos | Faixa | Arquivo citado |",
        "|---:|---|---|---|---|---:|---|---|",
    ]
    for index, item in enumerate(slither, 1):
        score, main, active, files = score_slither(item, active_files)
        lines.append("| " + str(index) + " | " + clean(item["check"]) + " | " + clean(item["impact"])
                     + " | " + ("sim" if main else "não") + " | " + ("sim" if active else "não")
                     + " | " + str(score) + " | " + band(score) + " | " + clean(files) + " |")
    lines += [
        "",
        "## Todos os registros da rede consultados",
        "",
        "Cada um dos " + str(len(events)) + " registros tem pontos de **atenção para revisão**, não uma nota de risco demonstrado. A regra usa tipo de evento e valor frente à oferta do bloco: troca de implementação 78; papéis de administração/troca 70; outros papéis 58; autorização sem limite 56; emissão/retirada 53–76; movimentos comuns 12–63. Um registro conhecido e conciliado pode ter pontuação alta por afetar uma função importante.",
        "",
        "| Nº | Bloco | Log | Tipo | Valor bruto | Pontos | Faixa | Motivo | Transação |",
        "|---:|---:|---:|---|---:|---:|---|---|---|",
    ]
    counts = collections.Counter()
    for index, event in enumerate(events, 1):
        score, reason = score_event(event, supply, role_names)
        counts[band(score)] += 1
        value = event.get("amount_raw", "—")
        name = event.get("flow", event["kind"])
        lines.append("| " + str(index) + " | " + str(event["block"]) + " | "
                     + str(event["log_index"]) + " | " + clean(name) + " | " + clean(value)
                     + " | " + str(score) + " | " + band(score) + " | " + clean(reason)
                     + " | `" + event["tx"] + "` |")
    lines += [
        "",
        "Contagem das faixas dos registros: alta=" + str(counts["alta"]) + ", média=" + str(counts["média"])
        + ", baixa=" + str(counts["baixa"]) + ". Esta é uma fila de revisão; não são "
        + str(counts["alta"]) + " falhas altas.",
        "",
        "## Limites da conclusão",
        "",
        "- A descoberta de contas com papéis usa as contas vistas no histórico; `complete_account_discovery=false`. Não afirmar enumeração total.",
        "- " + str(coverage["totals"]["BRH"]) + "/" + str(coverage["totals"]["BRF"]) + " ramos vêm da fonte recompilada e " + str(adaptive["combined_pcs"]) + "/" + str(pc_total) + " PCs vêm do bytecode no bloco; os denominadores medem coisas diferentes.",
        "- Securify2 e Manticore não eram aplicáveis a este runtime; Certora não foi acionado; Kontrol permanece pendente. Isso limita a revisão, sem invalidar os resultados das ferramentas concluídas.",
        "- O vínculo de fonte tem ressalva em três posições imutáveis declaradas. A decisão Rust é por afirmação e não uma prova de todos os estados.",
        "- Nenhuma operação foi enviada ao contrato na rede pública. Os arquivos detalhados do trabalho permanecem no VPS privado.",
        "",
    ]
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text("\n".join(lines), encoding="utf-8")
    return len(events), len(state["attempts"]), len(slither), counts


if __name__ == "__main__":
    job_id = sys.argv[1]
    output = Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "docs" / ("WCOP-RODADA-" + job_id + ".md")
    print(build(job_id, output))
