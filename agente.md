# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source é a entrada atual; bytecode virá depois. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0, M1 e M2 concluídos no recorte source-first de um arquivo sem imports: extração source/AST, grafo estrutural parcial, composição de modifiers locais e herdados, alcance interno entre contratos da mesma fonte e fluxo de valor conservador implementados para um arquivo Solidity por vez.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato tentativo quando o alvo é desconhecido. `ProxyRelation` só liga implementação local criada no construtor e guardada em endereço `immutable`, com spans da atribuição, tipo e receptor.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Modifiers definidos no mesmo arquivo são ligados pela referência de declaração do AST. A IR guarda invocação, definição, bindings parâmetro→argumento e efeitos sintáticos em `applied_modifiers`. Com exatamente um `_`, o CFG percorre os modifiers na ordem declarada e seus pós-trechos em ordem inversa, inclusive após `return` do corpo da função.
- `ReachabilityPath` registra chamadas internas do mesmo arquivo, inclusive bases até leituras/escritas de storage, candidatos de oracle e conversões, preservando o span de cada chamada. Alcance não comprova fluxo de valor.
- `ValueFlow` liga origem (parâmetro, storage, oracle, chamada externa) a guarda, retorno, argumento de conversão ou escrita de storage; conserva spans e chamadas contribuintes com confiança inferida. Propaga locals, argumentos/retornos internos e atualizações de storage. Não é execução simbólica.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- Fechamento M2: `crates/security-engine/src/{source,ir,lib}.rs`, fixture `ResolvedProxyFixture`, teste de alvo imutável/mutável, README e documentação.
- Herança no source AST: `crates/security-engine/src/{source,callgraph,dataflow,ir,lib}.rs`, fixture `InheritedVaultFixture` e teste de percurso oracle→conversão→supply.
- `README.md`, `agente.md`, `codex/agent.md` e `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- Fechamento M2: `cargo fmt --all`, `cargo test -p security-engine --offline` e `git diff --check`: 22 testes passaram, 0 falhas. CLI com solc local 0.8.37 executada nas fixtures de herança e proxy fixo.
- `cargo test --workspace --offline`: 22 testes passaram. Clippy com `-A clippy::collapsible_if -D warnings` passou; a execução estrita sem essa exceção encontrou 13 avisos de estilo `collapsible_if` já fora do critério funcional M2.
- Após herança: `cargo fmt --all` e `cargo test -p security-engine --offline`: 20 testes passaram.
- CLI executada também em `VaultWithOracle.sol` e `VaultCallFlowFixture.sol`; a primeira fixture liga oracle apenas à guarda, a segunda ao argumento de `convertToShares` e à escrita de `totalSupply`.

## Resultados
M2 liga base e derivado pelo AST, conserva identidade das chamadas herdadas e distingue alvo de proxy fixo local de endereço externo incerto. Fluxo de valor separa `oracle.latestPrice → require` (sem caminho às shares) de `oracle.latestPrice → readPrice → deposit → convertToShares → totalSupply` na fixture de preço. Os modifiers locais com um `_` também propagam parâmetros à guarda. São inferências estáticas conservadoras, não achados de vulnerabilidade.

## Erros conhecidos / limites
Sem suporte a imports, herança em outros arquivos, CFG completo, resolução de endereço de proxy fornecido na implantação ou bytecode. Loops têm aproximação de uma iteração com incerteza; chamadas desconhecidas não têm retorno presumido. Fluxo de valor não prova factibilidade de caminho. Modifiers ausentes da fonte ou com zero/múltiplos `_` ficam sem composição. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo.

## Próximo passo
Iniciar M3: recuperação conservadora de bytecode EVM, sem misturar inferência de bytecode com fatos de source.

## Decisão de ferramentas após M2 (2026-10-18)
- Avaliação documentada em `docs/TOOLING-STRATEGY.md`. Nenhum scanner, fuzzer ou provador adicional foi instalado ou integrado nesta etapa.
- Prioridade: Rust + Slither para triagem; Foundry para invariantes e reprodução; Halmos em propriedades curtas. Echidna **ou** Medusa apenas se houver lacuna de cobertura ou achados únicos. Mythril pode apoiar M3 seletivamente; Securify2 não é prioridade e Manticore está arquivado.
- Certora e Kontrol são opções para provas de alto impacto. O CLI padrão de Certora envia jobs aos servidores do fornecedor; alvo privado exige autorização específica antes de qualquer envio.
- Redução de uso de IA depende de gates e deduplicação; medir tokens, candidatos e bugs reproduzidos. Não afirmar ganho numérico sem benchmark. IA recebe apenas slices de candidatos ainda ambíguos.
- Esta etapa alterou documentação e memórias; não alterou código nem executou ferramentas de auditoria em alvos.

## Pendências
M3 bytecode, detectores econômicos, deduplicação, LLM e Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.
