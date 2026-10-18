# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0 e M1 concluídos. M2 em andamento: extração source/AST e grafo estrutural parcial implementados para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato de implementação com confiança tentativa, sem afirmar relação de proxy comprovada.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- `crates/security-engine/src/{cfg,ir,lib,source}.rs` e `crates/security-engine/tests/source_reconstruction.rs`.
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 11 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 nos três fixtures: `deposit` 12 arestas, `redeem` 7, `convertToShares` 5, `previewDeposit` 2 e `fallback` 4.

## Resultados
AST solc → IR automática para deposit/redeem, conversões, preview, leituras/escritas diretas, chamadas, condições e grafo estrutural parcial. Testes verificam separação entre retorno antecipado e continuação, falha de `require`, bifurcação e reunião de ternário e proveniência das arestas. `delegatecall` segue sinalizado com incerteza explícita.

## Erros conhecidos / limites
Sem suporte a imports, herança, CFG completo, corpos de modifiers, dataflow transitivo, resolução real da implementação de proxy ou bytecode. `for`, `do while`, `break`, `continue`, `try`, assembly e ternários múltiplos/nested não têm fluxo completo; suas lacunas ficam explícitas. Classificação por nome de função é inferida, não prova semântica econômica.

## Próximo passo
Concluir M2: expandir o grafo nas construções restantes e resolver dependências herdadas/modifiers; rastrear caminhos entre oracle e conversões sem perder proveniência; depois melhorar a resolução de proxy com evidência local.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
