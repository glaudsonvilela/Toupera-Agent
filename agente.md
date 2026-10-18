# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0 e M1 concluídos. M2 em andamento: extração source/AST, grafo estrutural parcial e observações de modifiers locais implementados para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato de implementação com confiança tentativa, sem afirmar relação de proxy comprovada.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Modifiers definidos no mesmo contrato são ligados pela referência de declaração do AST. A IR guarda invocação, definição e efeitos sintáticos separados em `applied_modifiers`, além de incluir suas dependências diretas nas listas da função. O placeholder `_` ainda não é intercalado ao CFG.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- `crates/security-engine/src/{ir,lib,source}.rs` e `crates/security-engine/tests/source_reconstruction.rs`.
- `tests/fixtures/ModifierFlowFixture.{sol,ast.json}`.
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 15 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 em `VaultFlowFixture.sol` e `ModifierFlowFixture.sol`; `previewDeposit` recebeu a leitura e a condição de `active`, e `deposit` recebeu um `AppliedModifier` com duas condições e incertezas explícitas.

## Resultados
Os modifiers locais agora contribuem leituras, escritas, chamadas, aritmética e condições diretamente observadas, com spans da definição e da invocação. As fixtures confirmam guarda `require`, guarda `assert`, escrita pós-placeholder, argumento ainda não vinculado ao parâmetro e incerteza explícita quando a definição ou o placeholder falta.

## Erros conhecidos / limites
Sem suporte a imports, herança, CFG completo, dataflow transitivo, resolução real da implementação de proxy ou bytecode. Os efeitos de modifiers são observações sintáticas: ainda não há ligação do `_` ao CFG nem binding entre argumentos e parâmetros; modifiers herdados ficam sem resolução. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo. Classificação por nome de função é inferida, não prova semântica econômica.

## Próximo passo
Concluir M2: intercalar modifiers no CFG e vincular argumentos/parâmetros; resolver dependências herdadas e caminhos entre oracle e conversões sem perder proveniência; depois melhorar a resolução de proxy com evidência local.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
