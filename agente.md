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
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- `crates/security-engine/src/cfg.rs` e `crates/security-engine/tests/source_reconstruction.rs`.
- `tests/fixtures/VaultLoopFixture.{sol,ast.json}`.
- `agente.md`, `codex/agent.md`, `docs/{ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 13 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 na fixture de laços, incluindo laço aninhado; todas as quatro funções produziram grafo sem incertezas.

## Resultados
O grafo agora diferencia inicialização, condição e atualização de `for`; condição pós-corpo de `do while`; caminhos de `break` e `continue`; e destinos próprios em laços aninhados. Os testes confirmam que `break` pula o restante do laço e `continue` volta ao ponto correto.

## Erros conhecidos / limites
Sem suporte a imports, herança, CFG completo, corpos de modifiers, dataflow transitivo, resolução real da implementação de proxy ou bytecode. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo; suas lacunas ficam explícitas. Classificação por nome de função é inferida, não prova semântica econômica.

## Próximo passo
Concluir M2: resolver dependências herdadas/modifiers e caminhos entre oracle e conversões sem perder proveniência; depois melhorar a resolução de proxy com evidência local. Expandir `try`/assembly apenas com semântica comprovável.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
