# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0 e M1 concluídos. M2 em andamento: extração source/AST, grafo estrutural parcial, composição de modifiers locais e herdados, alcance interno entre contratos da mesma fonte e fluxo de valor conservador implementados para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato de implementação com confiança tentativa, sem afirmar relação de proxy comprovada.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Modifiers definidos no mesmo arquivo são ligados pela referência de declaração do AST. A IR guarda invocação, definição, bindings parâmetro→argumento e efeitos sintáticos em `applied_modifiers`. Com exatamente um `_`, o CFG percorre os modifiers na ordem declarada e seus pós-trechos em ordem inversa, inclusive após `return` do corpo da função.
- `ReachabilityPath` registra chamadas internas do mesmo arquivo, inclusive bases até leituras/escritas de storage, candidatos de oracle e conversões, preservando o span de cada chamada. Alcance não comprova fluxo de valor.
- `ValueFlow` liga origem (parâmetro, storage, oracle, chamada externa) a guarda, retorno, argumento de conversão ou escrita de storage; conserva spans e chamadas contribuintes com confiança inferida. Propaga locals, argumentos/retornos internos e atualizações de storage. Não é execução simbólica.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- Herança no source AST: `crates/security-engine/src/{source,callgraph,dataflow,ir,lib}.rs`, fixture `InheritedVaultFixture` e teste de percurso oracle→conversão→supply.
- `crates/security-engine/src/{dataflow,ir,lib,source}.rs` e `crates/security-engine/tests/source_reconstruction.rs`.
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.
- `README.md`, `agente.md`, `codex/agent.md`, `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- Após herança: `cargo fmt --all` e `cargo test -p security-engine --offline`: 20 testes passaram.
- `cargo fmt --all` e `cargo test -p security-engine --offline`: 19 testes passaram; 0 falhas; doc-tests 0.
- CLI executada com solc local 0.8.37 em `VaultWithOracle.sol` e `VaultCallFlowFixture.sol`; a primeira fixture liga oracle apenas à guarda, a segunda ao argumento de `convertToShares` e à escrita de `totalSupply`.

## Resultados
Fluxo de valor separa `oracle.latestPrice → require` (sem caminho às shares) de `oracle.latestPrice → readPrice → deposit → convertToShares → totalSupply` na fixture de preço. Os modifiers locais com um `_` também propagam parâmetros à guarda. São inferências estáticas conservadoras, não achados de vulnerabilidade.

## Erros conhecidos / limites
Sem suporte a imports, herança em outros arquivos, CFG completo, resolução real da implementação de proxy ou bytecode. Loops têm aproximação de uma iteração com incerteza; chamadas desconhecidas não têm retorno presumido. Fluxo de valor não prova factibilidade de caminho. Modifiers ausentes da fonte ou com zero/múltiplos `_` ficam sem composição. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo.

## Próximo passo
Concluir M2: identificar implementação de proxy somente com evidência local; manter imports, bytecode e construções não cobertas explicitamente fora do escopo/como incertas.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
