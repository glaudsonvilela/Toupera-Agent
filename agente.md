# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0 e M1 concluídos. M2 em andamento: extração source/AST, grafo estrutural parcial, composição de modifiers locais e alcance interno de chamadas implementados para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato de implementação com confiança tentativa, sem afirmar relação de proxy comprovada.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Modifiers definidos no mesmo contrato são ligados pela referência de declaração do AST. A IR guarda invocação, definição, bindings parâmetro→argumento e efeitos sintáticos em `applied_modifiers`. Com exatamente um `_`, o CFG percorre os modifiers na ordem declarada e seus pós-trechos em ordem inversa, inclusive após `return` do corpo da função.
- `ReachabilityPath` registra chamadas internas do mesmo contrato até leituras/escritas de storage, candidatos de oracle e conversões, preservando o span de cada chamada. Alcance não comprova fluxo de valor.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- `crates/security-engine/src/{callgraph,ir,lib,source}.rs` e `crates/security-engine/tests/source_reconstruction.rs`.
- `tests/fixtures/VaultCallFlowFixture.{sol,ast.json}`.
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 17 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 em `VaultCallFlowFixture.sol`: `deposit` gerou 6 caminhos internos e 0 incertezas; `readPrice` gerou 2 caminhos.

## Resultados
`deposit → readPrice → rawPrice` alcança `oracle.latestPrice`; `deposit → convertToShares` alcança a conversão e leituras de `totalSupply`/`totalAssets`. Cada chamada e terminal conserva a proveniência. Os caminhos são sintáticos; não demonstram que o preço altera shares.

## Erros conhecidos / limites
Sem suporte a imports, herança, CFG completo, propagação de valor entre funções, resolução real da implementação de proxy ou bytecode. Caminhos internos apenas indicam alcance. Modifiers herdados ou com zero/múltiplos `_` ficam sem composição e com incerteza explícita. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo.

## Próximo passo
Concluir M2: provar propagação local e entre funções de oracle até conversões/storage quando existir, sem confundir alcance com valor; resolver herança e proxy com evidência local.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
