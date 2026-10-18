# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source e runtime EVM legado explícito já têm recortes de reconstrução separados. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0–M3 concluídos nos recortes documentados. M4 implementa triagem econômica source-backed, com calibração em fixtures adiada a pedido do usuário. M5 produz slices locais limitados e cache por conteúdo. M6 prepara requisições reduzidas e inclui adaptador DeepSeek opt-in, ainda sem chave ou chamada real. M7 prepara casos Foundry privados e possui juiz local com duas execuções, ainda sem harness de alvo ou execução. M8 prepara pacote privado e gate de entrega, sem relatório real. O motor não atribui identidade de oracle/assets/shares apenas pelo bytecode.

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
Sem suporte a imports, herança em outros arquivos, CFG completo ou resolução de endereço de proxy fornecido na implantação. O suporte a bytecode é somente runtime legado conservador, não EOF/creation code nem decompilação completa. Loops de source têm aproximação de uma iteração com incerteza; chamadas desconhecidas não têm retorno presumido. Fluxo de valor não prova factibilidade de caminho. Modifiers ausentes da fonte ou com zero/múltiplos `_` ficam sem composição. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo.

## Próximo passo
Prosseguir com a política de campanhas adicionais M7 (Echidna/Medusa/Halmos) e revisão do fluxo completo antes da validação geral. A instrumentação de LCOV, fuzz e contadores de invariant handlers existe, mas chamadas econômicas válidas ao alvo exigem harness e revisão humana. Não marcar achados como confirmados antes de reprodução e revisão. Testes, fixtures e benchmark permanecem adiados a pedido do usuário até a implementação geral estar completa.

## Etapa M3 inicial (2026-10-18)
- `crates/security-engine/src/bytecode.rs` analisa runtime EVM legado em hex. Remove metadados CBOR Solidity somente com estrutura completa e chave reconhecida; decodifica `PUSH` sem confundir dados imediatos com opcodes, cria blocos, resolve apenas saltos literais a `JUMPDEST`, reconhece candidatos de dispatch direto `PUSH4/EQ/PUSH/JUMPI` (`Inferred` só com `DUP1` anterior e alcance conhecido) e observa storage/transient storage/chamadas/criação/autodestruição por PC.
- `entry_summaries` reúne efeitos alcançados por cada selector recuperado. Blocos não alcançados pelo CFG conhecido mantêm confiança `Tentative`; saltos dinâmicos, `PUSH` truncado, opcodes desconhecidos e disponibilidade dependente de hard fork ficam em `unresolved`. Um slot constante só é ligado a `SLOAD`/`SSTORE`/`TLOAD`/`TSTORE` quando vem de `PUSH` imediatamente anterior, com confiança `Inferred`.
- CLI: `--bytecode <runtime.hex>` e `--compare-bytecode <runtime.hex> <solc.json> <source> <contrato>`. A comparação opcional usa runtime normalizado e `methodIdentifiers` de saída local do solc; selector isolado não comprova identidade do source. EOF, creation code, saltos calculados, stack/memória e fluxo econômico não são recuperados neste recorte.
- Documentação: `docs/BYTECODE-RECONSTRUCTION.md`, roadmap, IR e README atualizados. `cargo fmt --all`, `cargo check -p security-engine --offline` e `git diff --check` executados; nenhum teste automatizado foi executado nesta etapa.
- A revisão `3d2a9a6` foi enviada ao GitHub privado, sincronizada no VPS e compilada em release com `cargo build --release --locked -p security-engine` sob o usuário `toupera`; `.deployed-commit` registrou a revisão. O CLI de bytecode não foi executado em fixture nesta etapa.

## Decisão de ferramentas após M2 (2026-10-18)
- Avaliação documentada em `docs/TOOLING-STRATEGY.md`. Na etapa original de avaliação, nenhum scanner, fuzzer ou provador adicional havia sido instalado ou integrado; a instalação posterior no VPS está registrada abaixo.
- Prioridade: Rust + Slither para triagem; Foundry para invariantes e reprodução; Halmos em propriedades curtas. Echidna **ou** Medusa apenas se houver lacuna de cobertura ou achados únicos. Mythril pode apoiar M3 seletivamente; Securify2 não é prioridade e Manticore está arquivado.
- Certora e Kontrol são opções para provas de alto impacto. O CLI padrão de Certora envia jobs aos servidores do fornecedor; alvo privado exige autorização específica antes de qualquer envio.
- Redução de uso de IA depende de gates e deduplicação; medir tokens, candidatos e bugs reproduzidos. Não afirmar ganho numérico sem benchmark. IA recebe apenas slices de candidatos ainda ambíguos.
- Esta etapa alterou documentação e memórias; não alterou código nem executou ferramentas de auditoria em alvos.

## Infraestrutura informada pelo usuário
- VPS Hostinger KVM 4 `srv2074743.hstgr.cloud`, Ubuntu 26.04.1 LTS, 4 vCPU, 16 GiB RAM e 200 GB de disco; acesso SSH root verificado com a chave pública ED25519 autorizada pelo usuário.
- DeepSeek será usado por API. Inferência não será hospedada no VPS; enviar apenas slices mínimos e nunca credenciais ou o repositório inteiro por padrão.
- Pacotes Ubuntu atualizados. Usuário de execução `toupera` sem sudo, UFW restrito a OpenSSH, Fail2ban SSH e swap de 16 GB ativos. Atualizações automáticas de segurança já estavam ativas.
- Reinicialização pós-atualização concluída; SSH, Fail2ban, UFW, swap, `solc`, Rust, Slither, Foundry e binário Rust verificados de novo. Nenhuma reinicialização pendente.
- Rust/Cargo 1.93.1, solc 0.8.37 com SHA-256 conferido, Slither 0.11.6 e Foundry 1.8.5 instalados. Fonte privada enviada por `git archive` a `/srv/toupera-agent`, sem token GitHub; CLI Rust compilado em release com `--locked`.
- Smoke operacional: CLI produziu 4 registros de IR em `VaultCallFlowFixture.sol`; Slither analisou a fixture com 102 detectores e 3 alertas (`success=true`); `forge --version` funcionou. Não foram executados testes automatizados nesta etapa de infraestrutura.
- Nenhum serviço web, API, integração DeepSeek ou secret foi implantado. O SSH por senha/root existente permaneceu habilitado; hardening adicional de autenticação exige coordenação para evitar perda de acesso. Ver `docs/VPS-SETUP.md`.
- Limitar paralelismo de fuzzing e escalonar provas simbólicas conforme métricas reais de CPU, memória e disco.

## Instalação adicional de ferramentas (2026-10-18)
- A pedido do usuário, os 10 executores da estratégia estão disponíveis: Slither 0.11.6, Mythril 0.24.8, Securify2 (commit `1913dfe`), Manticore 0.3.7, Foundry 1.8.5, Echidna 2.3.3, Medusa 1.5.1, Certora CLI 8.20.2, Halmos 0.3.3 e Kontrol 1.0.255. Inventário e métodos em `docs/VPS-SETUP.md`.
- Python 3.10 e 3.12 isolados por uv; Java 21 para Certora; Podman rootless para Securify2, Manticore e Kontrol. Securify2 compilou com `graphviz==0.19.2` fixado apenas na cópia da ferramenta e com solc 0.5.12. Manticore e Securify2 são legados; não presumir compatibilidade com contratos modernos.
- Comandos de versão/ajuda foram executados para os CLIs; Securify2 foi construído e abriu `--help`. Nenhum teste automatizado do Toupera Agent foi executado. Nenhuma análise em alvo real, prova Certora ou envio de source privado a terceiros foi realizado.
- O CLI Certora não tem `CERTORAKEY`; instalação não habilita provas remotas. As ferramentas ainda não estão ligadas à IR Rust ou ao pipeline do projeto.

## Fechamento M3 — recorte legado (2026-10-18)
- `crates/security-engine/src/bytecode_flow.rs` propaga stack e palavras de memória de 32 bytes por blocos e arestas conhecidas, com limites de iteração, origens e fluxos. Resolve destinos constantes adicionais de `JUMP`/`JUMPI` e registra relações PC→PC entre calldata, callvalue, storage, retorno/sucesso de chamada e balance até storage, valor/alvo/entrada de chamada, retorno e condição; PCs aritméticos acompanham a dependência.
- `BytecodeAnalysis.value_flows` e `BytecodeEntrySummary.value_flows` são inferências estáticas. No resumo por selector, confiança é `Tentative` por possível compartilhamento de blocos. Memória parcial, ABI desconhecida, salto dinâmico, stack incompatível, opcodes não modelados e orçamento esgotado são incertezas ou limites; não há detecção de bug nesta etapa.
- `compare_with_solc_contract` atribui `EconomicOperation` por nome de método somente quando o runtime normalizado do solc coincide byte a byte. Um selector isolado nunca recebe rótulo econômico de source divergente. Documentação e checklist M3 atualizados em `docs/BYTECODE-RECONSTRUCTION.md` e `docs/ROADMAP-CHECKLIST.md`.
- `cargo fmt --all` e `cargo check -p security-engine --offline` executados durante a implementação. Nenhum teste foi adicionado ou executado nesta etapa.

## Pendências
Calibração M4–M6, execução e métricas do juiz Foundry M7 e entrega M8 permanecem pendentes. Nunca armazenar secrets ou executar exploração em produção.

## Complemento M3 — EVMole como evidência independente (2026-10-18)
- Dependência Rust `evmole = =0.9.4`, com recursos padrão desligados, integra a análise de runtime legado já normalizado em `crates/security-engine/src/bytecode_evmole.rs`. `BytecodeAnalysis.evmole` registra selectors ABI/fallback, tipos prováveis de argumentos, mutabilidade, layout de storage persistente/transitório, CFG, versão, confiança e divergências. O processamento é local; não há consulta a rede durante a análise.
- Propostas do EVMole não entram automaticamente em `control_edges`, `effects` nem `value_flows` nativos. Selector só é aceito se sua entrada aponta a um `JUMPDEST` decodificado; tipos, storage e arestas são `Tentative`. Slots EVMole só ganham `corroborating_access_pcs` quando um efeito nativo com slot literal e selector alcançável coincide. Ausência de PC de corroboração não prova ausência do acesso.
- `compare_with_solc_contract` pode considerar selector ABI exclusivo do EVMole; `recovered_by` mostra `native`, `evmole` ou ambos. EVMole exclusivo e divergência de destino mantêm confiança `Tentative`; rótulo econômico pelo nome do método exige igualdade do runtime compilado normalizado. A comparação agora normaliza o runtime do solc sem reexecutar toda a análise.
- O companion registra motivo de omissão acima de 24.576 bytes e preserva a análise nativa caso o EVMole falhe. EVMole não confirma vulnerabilidades nem prova ABI/layout.
- `cargo check -p security-engine --offline` e `cargo fmt --all` foram usados para compilação e formatação. Nenhum teste automatizado foi adicionado ou executado nesta etapa.

## Complemento M3 — Heimdall e Gigahorse (2026-10-18)
- `bytecode_companions.rs` importa `abi.json` do Heimdall e `contract.tac` do Gigahorse em `CompanionAnalysis`, separado de `BytecodeAnalysis` nativo. O CLI `--bytecode-companions <runtime.hex> <abi.json> <contract.tac>` correlaciona selectors Heimdall com PCs de entrada nativos/EVMole e blocos/sucessores TAC com PCs de instrução decodificados. Toda inferência importada é `Tentative`; assinaturas ABI e expressões TAC não viram fatos econômicos nem achados.
- `scripts/run-bytecode-companions.sh` executa Heimdall sobre arquivo local com `--skip-resolving` e Gigahorse na imagem oficial via Podman sem rede, com limites de tempo e saída persistida em diretório novo. A execução é opcional; `--bytecode` e `--compare-bytecode` não dependem desses CLIs. O wrapper falha se faltar output, preservando os artefatos para inspeção. Leia `docs/BYTECODE-RECONSTRUCTION.md`.
- `cargo fmt --all`, `cargo check --locked --offline -p security-engine` e `git diff --check` executados. Nenhum teste automatizado foi adicionado ou executado nesta etapa.
- No VPS, `/usr/local/bin/heimdall` foi instalado do asset oficial da release 0.9.3 (o binário informa 0.9.2), SHA-256 `8d58c97f56d584d5d0737d0d0895fc8c68b7b8b2b680f2a7842245789f0b4868`; a imagem oficial Gigahorse está no Podman de `toupera` pelo digest `sha256:f676ca8aaf88acd47be27ed1967acddc9c99acdd041b34e79472cfb028910743`. CLI `--help` do contêiner abriu; não houve decompilação de contrato nesta instalação.
- A revisão de implementação `a3b4d6f` foi enviada ao GitHub privado, sincronizada no VPS e compilada em release com `cargo build --release --locked -p security-engine`. O marcador `.deployed-commit` confere com a revisão sincronizada; nenhuma execução de decompilação ponta a ponta foi feita nesta etapa.

## M4 — triagem econômica determinística (2026-10-18)
- `crates/security-engine/src/economic_analysis.rs` analisa a IR source-first de um arquivo e produz `EconomicAnalysisReport` com candidatos `UNVERIFIED`, confiança `Tentative`, regras, premissas e source spans. Regras cobrem lacunas estruturais de conservação assets/shares, assimetrias de accounting e preço, preview/execução, rounding, fees, escalas decimais, oracle e sensibilidade de primeira emissão/doação. `source.rs` agora guarda chamadas explícitas `mulDiv`/`ceilDiv` como expressões aritméticas para reconhecer direções de arredondamento. São sinais sintáticos condicionais, nunca prova de vulnerabilidade, conservação numérica ou lucro.
- O CLI `--economic-candidates <source.sol> [solc-path] [slither.json]` importa opcionalmente JSON local do Slither. Todos os detectores ficam visíveis como sinais; só `divide-before-multiply`, `unchecked-transfer` e `arbitrary-send-erc20` têm mapeamento econômico inicial, exigindo função sem ambiguidade e span válido. Deduplicação por contrato, função e categoria conserva regras e evidências. `scripts/run-economic-candidates.sh` executa Slither local e salva source copiado, versões, JSON bruto e relatório; não foi executado nesta etapa.
- `metrics` registra sinais brutos, mapeamentos e candidatos fundidos. `false_positives_reviewed` fica `null` até calibração posterior, conforme pedido do usuário para testar só após implementação completa. O M4 está implementado nesse recorte source-backed, com medição de falsos positivos/cobertura ainda pendente. Leia `docs/ECONOMIC-ANALYSIS.md` e `docs/ROADMAP-CHECKLIST.md`.
- `cargo fmt --all`, `cargo check --locked --offline -p security-engine`, `bash -n` do wrapper e `git diff --check` usados para formatação/compilação/sintaxe; nenhum teste automatizado nem análise de fixture foi executado nesta etapa.
- A revisão de implementação M4 `d2ed831` foi enviada ao GitHub privado, sincronizada no VPS e compilada com `cargo build --release --locked -p security-engine` sob `toupera`; `.deployed-commit` registrou a revisão. Nenhum candidato foi avaliado em fixture nesta etapa.

## M5 — cache e contexto mínimo (2026-10-18)
- `context_package.rs` calcula SHA-256 de source, AST, função, compilador e binário do motor; runtime legado normalizado e JSON Slither entram quando fornecidos. A chave de cache inclui versão do formato e todas as entradas. Em acerto, pula compilação, IR, triagem M4 e montagem de slices. O cache local exige diretório `0700` e arquivos regulares `0600`; não armazena source integral nem outputs brutos das ferramentas.
- `--context-package <source.sol> <private-cache-dir> <solc-path> [--slither <json>] [--bytecode <runtime.hex>]` devolve JSON local com candidato `UNVERIFIED`, função e trechos relacionados limitados, evidências, storage, aritmética, oracle, chamadas, perguntas de invariantes, incertezas e `truncated`. O hash opcional de runtime não liga bytecode a source. Não há chamada à IA nem envio de repositório.
- Documentação em `docs/CONTEXT-PACKAGE.md`; roadmap, README, documentação M4/VPS e `codex/agent.md` atualizados. `cargo fmt --all`, `cargo check --locked --offline -p security-engine` e `git diff --check` executados; nenhuma fixture, teste automatizado ou benchmark executado, conforme pedido do usuário.
- Revisão de código M5 `acd4d48` enviada ao GitHub privado, sincronizada no VPS e compilada em release com `cargo build --release --locked -p security-engine` sob `toupera`. O marcador `.deployed-commit` recebeu essa revisão antes do complemento documental final.

## M6 — hipóteses de IA estruturadas (2026-10-18)
- `llm_review.rs` oferece `HypothesisProvider`, adaptador DeepSeek HTTPS e `CachingProvider`. `--llm-plan <context.json>` só prepara requisições locais; `--llm-review` requer `--provider deepseek`, `--allow-external`, `--review-cache` privado e chave `TOUPERA_DEEPSEEK_API_KEY` no ambiente. Não inserir chave em argumentos, arquivos do projeto ou logs.
- Seleção exige candidato `UNVERIFIED`, evidência, perguntas de invariantes e função com trecho. Deduplica por contrato/função/categoria/hash e monta payload apenas de campos permitidos do slice. O modelo devolve JSON com hipótese, premissas, caminhos e teste sugerido ou evidência insuficiente; parser valida a forma, não a verdade. Todo item continua `UNVERIFIED` até M7.
- Métricas de chamadas e tokens vêm da resposta da API; rate card local opcional permite estimativa de custo sem preço fixado em código. Respostas válidas idênticas são reaproveitadas em cache `0700`/`0600`. Limite padrão de 5 candidatos, máximo 20 por execução; sem retry automático.
- Documentação em `docs/LLM-REVIEW.md`, roadmap/README/arquitetura/VPS e `codex/agent.md` atualizados. `cargo fmt --all`, `cargo +1.93.0 check --locked --offline -p security-engine` e `git diff --check` usados. Nenhum teste, fixture, chamada DeepSeek ou benchmark executado, conforme pedido do usuário.
- Revisão de código M6 `6ff127a` enviada ao GitHub privado, sincronizada no VPS e compilada em release com `cargo build --release --locked -p security-engine` sob `toupera`. `.deployed-commit` recebeu essa revisão antes do complemento documental final. O diretório `web/` surgiu como arquivo não rastreado durante a etapa; não foi alterado, adicionado ou incluído no deploy.

## M7 — preparação e juiz Foundry (2026-10-18)
- `foundry_judge.rs` lê pacote M5, relatório M6 e arquivo source idêntico ao hash registrado. Para cada hipótese `UNVERIFIED`, cria caso privado com snapshot `src/Target.sol`, `case.json` e scaffold `test/Scenario.t.sol` que falha até receber um harness específico. Texto da IA não vira Solidity. `--foundry-plan` não executa teste.
- `--foundry-run` exige harness alterado, nota de revisão, confirmação de chamadas válidas e limites, binários absolutos de forge/solc, configuração Foundry local intacta e source com hash original. Faz duas execuções offline com seed fixa e fuzz de 256 entradas, guarda JSON e traces. Só oferece `REPRODUCED_PENDING_REVIEW` quando ambos os testes exigidos passam nas duas execuções; os demais casos são `INCONCLUSIVE`. Não confirma vulnerabilidade nem rejeita hipótese automaticamente.
- O gate humano não prova não vacuidade, e `--offline` não isola rede completamente. Executar apenas em ambiente local isolado e autorizado após revisão do harness. Cobertura, chamadas válidas, reverts, Echidna/Medusa, piloto Halmos e métricas seguem pendentes até os testes gerais pedidos pelo usuário. Ver `docs/FOUNDRY-JUDGE.md` e roadmap. Nenhum teste Foundry, fixture ou campanha foi executado nesta etapa.
- A revisão de implementação `b945a96` foi publicada no repositório privado e compilada em release no VPS sob `toupera`; `.deployed-commit` recebeu essa revisão antes deste complemento documental. Nenhuma chave ou chamada DeepSeek foi adicionada. O diretório local `web/` não rastreado permaneceu fora do commit e do deploy.

## M8 — evidência e entrega (2026-10-18)
- `evidence_package.rs` oferece `--evidence-plan <case-dir> <private-output-dir>` e `--evidence-finalize <bundle-dir>`. O plano copia source, harness, configuração e resultados Foundry disponíveis a pacote privado com SHA-256 por artefato. Sem duas reproduções positivas o estágio permanece `PENDING_REPRODUCTION`; com resultado M7 consistente, `PENDING_HUMAN_REVIEW`.
- `review.json` começa sem aprovações e exige pessoa revisora, referência de escopo autorizado, versão do alvo e ferramentas, ambiente, passos, impacto, condições, inferências, duplicidade e confirmação de asserções/evidências. Finalização valida hashes, identidade do caso, JSON das duas execuções e aprovações; só então grava `report.md` e `ready.json` com `REPORT_READY`. Não publica nem envia o relatório.
- O manifesto separa fatos automáticos, premissas e limitações; o relatório acrescenta inferências e impacto escritos por pessoa. Atestados humanos não provam sua própria veracidade. Esta etapa foi apenas compilada e formatada; nenhum pacote, teste, fixture, IA, benchmark ou relatório real foi executado. Leia `docs/EVIDENCE-DELIVERY.md` e roadmap. Preserve `web/` fora do commit e do deploy.
- A revisão M8 `ea48456` foi publicada no GitHub privado, sincronizada e compilada em release no VPS sob `toupera`. `.deployed-commit` recebeu essa revisão antes deste complemento documental. A compilação não executou os comandos M8 nem testes.

## Complemento M7 — métricas observáveis (2026-10-18)
- O juiz M7 extrai `kind.Fuzz.runs` de cada JSON Foundry; duas rodadas só podem produzir `REPRODUCED_PENDING_REVIEW` se o fuzz exigido registrar ao menos 256 entradas, todos os testes passarem e `forge coverage` gerar LCOV do alvo com pelo menos uma linha atingida. Salva `runs/coverage.info` e log privado; resume linhas e branches do `src/Target.sol`.
- Se houver testes `invariant*`, registra `kind.Invariant.calls` e `.reverts` por rodada. Esses contadores são de handlers da campanha, não comprovam chamadas econômicas válidas ao alvo. Sem campanha, ficam `null`. O M8 agora copia e confere JSON fuzz e LCOV antes de liberar revisão humana e relatório.
- Somente compilação/formatação Rust e diff estático nesta etapa; nenhum teste Foundry, cobertura, fixture, campanha, benchmark ou relatório real foi executado. Fontes do formato: `foundry-rs/foundry` tag `v1.8.5` e help do Forge instalado. `web/` não rastreado segue preservado.
- A revisão de código `d47a909` foi publicada no GitHub privado, sincronizada e compilada em release no VPS sob `toupera`. `.deployed-commit` recebeu a revisão antes deste complemento documental. Nenhuma campanha M7 ou pacote M8 foi executado no servidor.
