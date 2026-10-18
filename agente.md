# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0 e M1 concluídos. M2 em andamento: primeiro recorte de extração source/AST implementado para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato de implementação com confiança tentativa, sem afirmar relação de proxy comprovada.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- `Cargo.lock`, `crates/security-engine/Cargo.toml`, `crates/security-engine/src/{lib,ir,source,main}.rs`
- `crates/security-engine/tests/source_reconstruction.rs`
- `tests/fixtures/{VaultWithOracle,VaultProxyFixture,VaultFlowFixture}.ast.json` e os dois novos fixtures `.sol`
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ARCHITECTURE,ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`

## Testes executados
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 9 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 nos três fixtures: IR gerada para vault, proxy e fluxo interno.

## Resultados
AST solc → IR automática para deposit/redeem, conversões, preview, leituras/escritas diretas, chamadas e condições. Source spans, erros do compilador e IDs de arquivo são verificados. `delegatecall` é sinalizado com incerteza explícita.

## Erros conhecidos / limites
Sem suporte a imports, herança, CFG completo, corpos de modifiers, dataflow transitivo, resolução real da implementação de proxy ou bytecode. Classificação por nome de função é inferida, não prova semântica econômica.

## Próximo passo
Concluir M2: construir grafo de fluxo sobre AST, resolver dependências herdadas/modifiers e rastrear caminhos entre oracle e conversões sem perder proveniência; depois melhorar a resolução de proxy com evidência local.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
