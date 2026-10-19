# Instruções para futuras sessões do Codex

## Homepage pública — Toupeira Agente

- O usuário autorizou criar e publicar a homepage em `toupeiraagente.tech` e pediu corrigir a marca para **Toupeira Agente**. O nome histórico do repo não foi alterado.
- Código estático em `homepage/`, com imagem ImageGen, fumaça/partículas Canvas, luz, parallax, abas por teclado e pausa de animações. Preservar `web/` não rastreado. Nenhum endpoint público para o motor ou seus relatórios foi criado.
- Conferidos desktop/mobile, botões, teclado e pausa. Caddy preparado para servir apenas os arquivos públicos em `/var/www/toupeiraagente/current`; registrar estado final de HTTPS depois da publicação. Ver `homepage/README.md`.
- Homepage publicada: HTTPS 200 em IPv4/IPv6, certificados Let's Encrypt, HTTP→HTTPS 308 e www→apex 301. Caddy 2.11.7 ativo/habilitado, TCP 80/443 liberadas no UFW. Release web `04a1473` em `/var/www/toupeiraagente/releases/`, selecionada por `current`; marcador web separado em `/var/www/toupeiraagente/.deployed-commit`. Produção conferida no navegador sem erros, assets 200 e caminho privado 404. Não confundir publicação da homepage com disponibilização pública do motor Rust.

## Objetivo ativo de engenharia forense

- O usuário exige que o Rust automatize a reconstrução histórica, a análise de transações e estado, invariantes, simulação e retorno da quarentena ao mapa, sem depender da IA para criar o contexto. O mapa/sondagem atual é triagem; não chamá-lo de scanner forense completo.
- Atualizar `agente.md` e `docs/ROADMAP-CHECKLIST.md` ao concluir cada etapa. Marcar cobertura parcial e falhas de RPC explicitamente; artefatos de alvo em `private-output/`; preservar `web/` não rastreado.
- Pedido vigente: executar simulações somente no VPS, em processo autônomo, e verificar o resultado. Não simular no ambiente local desta sessão nem confundir leitura histórica ou `eth_call` com replay de execução.
- Preferência de comunicação vigente: usar termos simples e neutros com o usuário, como “cenários locais”, “checagem”, “mapa” e “diferença observada”, preservando precisão técnica sem linguagem alarmista.

## Resumo de caminhos e segunda leitura por variável

- O Rust `--local-coverage` já roda no VPS sobre `private-output/real-targets/yield-vault/local-scenarios-v7/report.json`, sem novo cenário/RPC. `local-coverage-v2.json`: 24 casos, 110 PCs `JUMPI` estáticos, 50 observados, 58 resultados; zero em quatro chamadas já observa 56, 16 positivos comuns não acrescentam resultado, dois casos acima de limite acrescentam dois. Atribuição por operação é pelo nome da chamada principal, sem PC→função comprovado.
- Leia `docs/VARIABLE-LENS.md`: fontes oficiais Solidity/Slither, SAILFISH e Gigahorse, grafo de variáveis e fatias nos dois sentidos. Os itens concluídos e pendentes constam da seção seguinte. Preservar `web/` não rastreado e atualizar este arquivo e `agente.md` em cada etapa.

## Primeiro recorte de variáveis executado no VPS

- `--variable-lens` compila AST + storageLayout do source de análise, valida hashes/identidade do relatório forense e `local-coverage`, e produz grafo sintático de atribuições, usos e fatias transitivas. No VPS, `variable-lens-v4.json`: 742 declarações, 180 arestas, 7 estados persistentes, 2 `immutable`. `depositCap` recebe `cap_`/`newCap` e alimenta `cap` em `maxDeposit` na AST achatada.
- O bundle verificado oferece source map dos PCs: 18/50 faixas amplas foram deliberadamente deixadas sem variável; 17/50 PCs têm referência candidata em faixa curta, marcada tentativa. Não misturar IDs da AST achatada e verificada nem afirmar que uma referência em faixa controla o PC. Sem novo fork/RPC/IA. Leia `docs/VARIABLE-LENS.md`; SSA consta abaixo, enquanto fluxo entre funções e novos cenários guiados por estado permanecem abertos. Atualizar ambos os arquivos de agente e o checklist depois de cada etapa. Preservar `web/` não rastreado.

## SlithIR SSA integrado

- `scripts/export-variable-ssa.py` exporta operações SSA com o Python do pipx Slither; `scripts/run-variable-lens.sh` produz SSA e relatório Rust em diretório privado. O Rust valida SHA-256 do source e faz correspondência por offset+nome de declaração. Arestas SSA têm `evidence_source=slither_ssa` e `confidence=inferred_ssa`; arestas AST permanecem separadas.
- VPS `variable-lens-ssa-v1/`: 1.303 operações, 287 relações SSA, 467 relações totais, 42 operandos posicionados não vinculados. Sem RPC/fork/IA; permissões `0700`/`0600`. Próximos gates: entre funções, mappings/arrays, runtime/source forte e novos estados. Atualizar `agente.md` e este arquivo após cada etapa, mantendo `web/` fora dos commits.

## Etapa histórica concluída, 2026-10-18

- O motor Rust usa `alloy-primitives::U256`; seleciona RPC com maior histórico após validar chain/bloco/runtime; varre logs do vault e transferências do asset desde o deploy em lotes, com erros e alcance explícitos; cruza transações/recibos, eventos, cap, owner, supply e balanço por bloco. Tenta `debug_traceTransaction`, registrando a lacuna quando não há acesso. Divergências observadas chegam à quarentena e ao mapa refeito; a saída inclui `coverage`.
- Execução de leitura já feita em YieldVault Sepolia: 818.846 blocos, 5 eventos do vault, 2 transferências do asset, 3 recibos, 1 bloco com eventos conferido, 108 chamadas de borda (103 retornaram). `coverage.evidence_gate=history_reconciled_trace_gaps`: nenhum rastro interno (RPC HTTP 429), 299 itens de bytecode não resolvidos, sem simulação de sequência ou prova de segurança. Ver `docs/REAL-EVM-PILOT.md` e `docs/FORENSIC-QUARANTINE.md`.
- Próximos gates: sequências locais em fork com cobertura mensurável, calibração M4, ramo positivo M5–M8. Não marcar esses itens como concluídos. Uma tentativa manual de replay anterior ao pedido recente terminou por timeout e não forneceu evidência; não repetir simulações no ambiente local. Código `67de987` publicado no Git privado e compilado no VPS sob `toupera`; sem testes nesta atualização.
- Replay autônomo autorizado para o VPS após aquela tentativa. `scripts/run-forensic-replay.sh` confere identidade, executa no máximo três `cast run --no-bal` com timeout de 240 s e salva rastros/resumo privados. Sucesso do `cast` não valida vulnerabilidade nem substitui sequência de operações em fork.
- Execução concluída: `toupera-forensic-replay-20261018.service` no VPS gerou 3/3 rastros locais com `Transaction successfully executed`, zero timeout/falha, serviço `Result=success`. Os rastros permanecem em `/srv/toupera-agent/private-output/real-targets/yield-vault/replay-vps-20261018/`; nenhuma transação foi transmitida à rede. Sequências inéditas e cobertura de ramos seguem pendentes. Ver `docs/REAL-EVM-PILOT.md` e `docs/VPS-SETUP.md`.
- Nova etapa: `--local-scenarios` em `local_scenarios.rs` abre Anvil filho em loopback depois de conferir identidade do relatório, gera cenários deposit/mint/withdraw/redeem e depósito→resgate, coleta estados, recibos, reversões e pontos `JUMPI`. Compilação local passou; execução no VPS ainda pendente neste registro. Os PCs são agregados entre chamadas e não dão cobertura percentual. Ver `docs/LOCAL-SCENARIOS.md`; atualizar este arquivo após a execução e antes de chamar a etapa concluída.
- Primeira execução no VPS: 18 cenários concluídos, zero reversões/erros, 72 posições `(depth,PC)` com `JUMPI`; depósito→resgate custou 1 unidade mínima do asset. O comparador gerou dois avisos falsos `-0`/`0`; correção implementada, repetir no VPS. `ForensicInput.local_scenario_report_path` agora injeta diferenças como pedidos de revisão na quarentena antes de refazer o mapa; não vira M5 sem anomalia observada. Ver `docs/LOCAL-SCENARIOS.md`.
- Segunda execução VPS: 18/18, zero diferenças e zero reversões; o erro `-0` acabou. A passagem `--forensic-probe` anexou o relatório local mas ficou `partial` porque o RPC histórico devolveu HTTP 429 antes da leitura de logs. Não usar esse relatório como cronologia completa. `Rpc::call` ganhou espaçamento HTTPS e backoff limitado para 429; repetir no VPS. A leitura completa anterior permanece evidência separada.
- Repetição com backoff concluída no VPS: 818.846 blocos, histórico do vault e asset completos, 3 recibos, 105/108 chamadas de borda, 18 cenários anexados, 29 zonas/19 quarentenas sem mudança de prioridade. `history_reconciled_trace_gaps` continua correto para rastros históricos do RPC. Novo código `--cached-reassessment` exige SHA-256 exato do relatório base e identidade completa, reaproveita a cronologia e reavalia todos os hotspots sem RPC. Métrica `taken`/`not_taken` de `JUMPI` adicionada ao cenário local; executar essas duas alterações no VPS antes de marcá-las verificadas.
- Primeira execução de resultados `JUMPI` concluiu 18 cenários, mas decodificou só 12 resultados para 72 posições: palavras hex ímpares do stack não eram aceitas. Parser corrigido com zero à esquerda; repetir no VPS e não citar os 12 como medida final. Round trip segue `-1`, sem diferença de estado.
- Execução final VPS: 18/18 cenários, zero reversões/falhas/diferenças, 72 posições `JUMPI`, 80 resultados, 8 posições nos dois sentidos; round trip `-1`. Contagens incluem vault+token e não são percentual de cobertura. `--cached-reassessment` validou SHA-256 do relatório base e identidade, reavaliou 29 zonas/19 quarentenas mantendo 818.846 blocos históricos e fez zero chamadas ao RPC; nenhuma prioridade mudou. Artefatos privados: `local-scenarios-v4/` e `cached-reassessment-v1/`. Próximos gates: cobertura por função/ramo com denominador, mais estados/atores, calibração M4 e ramo positivo M5–M8.
- Próxima medição implementada, ainda sem execução no VPS: contar PCs `JUMPI` no runtime verificado e observar só os do frame raiz (`depth=1`) dos cenários. Isso dá denominador estático por contrato, não prova alcance nem cobertura por função. Campos novos em `local_scenarios.rs`, importação e cache; conferir no VPS e atualizar números antes de declarar a etapa verificada.
- Medição VPS conferida: 110 PCs `JUMPI` estáticos no runtime do vault, 50 observados no frame raiz, 56 resultados (6 PCs nos dois sentidos); agregado vault+token 72/80/8. Reavaliação com cache importou as métricas e preservou 818.846 blocos sem RPC. Nova grade com zero e uma unidade acima de `maxWithdraw`/`maxRedeem` implementada, ainda precisa execução no VPS. Reversões de limite são resultados esperados, não candidatos automáticos.
- Primeira grade ampliada VPS: 24 casos, 22 concluídos, 2 reversões de limites, zero RPC falho. Dois avisos eram `-0` em deposit/mint de zero; `expected_deltas` agora usa uma única função de sinal para todos os quatro métodos. Repetir e só então registrar ausência de diferenças. Métrica vault 110 estáticos/50 vistos/58 resultados/8 PCs nos dois sentidos.
- Grade final `local-scenarios-v7`: 24 casos, 22 concluídos, 2 reversões de limites, zero falhas de RPC/diferenças; round trip `-1`. `cached-reassessment-v3` aceitou hash/identidade, conservou 29 zonas/19 quarentenas e fez zero RPC. Vault 110 PCs estáticos/50 vistos/58 resultados/8 PCs nos dois sentidos. Relatórios privados só no VPS.
- Ajuste M4 pendente de conferência no VPS: comparar divisão literal para baixo `x / d` e para cima `(x + d - 1) / d` entre preview/execução, exigindo mesmo numerador/denominador. Resultado continua `UNVERIFIED` e não comprova o ramo M4→M8.
- Calibração M4 no VPS concluída: 1 candidata `RoundingAsymmetry`/`UNVERIFIED` em `withdraw(uint256)` com 2 source spans na fixture M7. M5 produziu 1 slice e plano M6 offline 1 pedido, sem API. O source tem ramo alternativo compatível, portanto conferir `faultyPreview` por instância. Os casos M7 prévios eram manuais; ligação positiva completa até M8 segue aberta.

## Ciclo forense/quarentena, 2026-10-18

- `forensic_probe.rs` implementa: manifesto `forensic-input-v1` → source/runtime/Slither + RPC de bloco fixado verificado → mapa de zonas com score explicável → até 24 quarentenas de variáveis/caminhos → segunda sondagem read-only e cruzamento de evidências → reconstrução integral do mapa. Anomalia observada pode entrar em M5 como `UNVERIFIED` por `--forensic`; M5 rejeita source/runtime divergentes e inclui relatório na chave de cache. Consultar `docs/FORENSIC-QUARANTINE.md`.
- Primeira passagem YieldVault Sepolia: 29 zonas, 19 quarentenas, 78 sondagens, 15 relações, zero anomalias nas amostras; M5/M6 zero candidatos/requests. Seu histórico de eventos era parcial e foi ampliado na etapa histórica acima. Não chamar `sampled_no_anomaly_observed` de ausência de bugs. Sem transações ou DeepSeek. Testes Rust de etapa anterior: 27 passaram. Próximo gate positivo é anomalia real autorizada → hipótese M6 → Foundry M7 → revisão M8.
- O fallback `sepolia.gateway.tenderly.co` foi aceito depois de validar chain ID, bloco e runtime; o RPC primário ficou indisponível para estado histórico. Preservar `web/` não rastreado, e manter artefatos de alvo em `private-output/` ignorado.

1. Leia primeiro `/agente.md`.
2. Trabalhe em etapas pequenas.
3. Não faça grandes refactors sem necessidade.
4. Não generalize o escopo: priorize Solidity/EVM, Vaults e Oracles.
5. Trate o motor Rust como analisador econômico e motor de reconstrução do contrato; source e runtime legado já têm recortes implementados, mas toda inferência incerta deve permanecer explicitamente incerta.
6. Execute testes quando autorizados pelo escopo da etapa. O pedido de adiar testes gerais permanece para alvos reais; o piloto sintético M7 foi executado após a implementação local, conforme o pedido posterior de concluir M7. Registre exatamente o que foi medido.
7. Atualize `/agente.md` depois de cada etapa importante.
8. Nunca armazene API keys, tokens, passwords, secrets, private keys ou seed phrases.
9. Não modifique outros projetos no SSD Sherlock.
10. Prefira análise determinística antes de LLM e envie somente contexto mínimo.
11. Minimize tokens e contexto.
12. Não invente resultados: registre apenas verificações realmente executadas.
13. Uma vulnerabilidade só é válida após reprodução.
14. Não execute exploração em produção nem envie transações ofensivas para contratos reais.

## Estado após M2 — recorte source-first concluído

- Leia `docs/ROADMAP-CHECKLIST.md`, `docs/ECONOMIC-IR.md` e `docs/SOURCE-RECONSTRUCTION.md`.
- `source.rs` extrai IR de solc AST para um arquivo sem imports e resolve modifiers locais e herdados do mesmo arquivo. `cfg.rs` intercala modifiers com um `_`. `callgraph.rs` produz `ReachabilityPath`. `dataflow.rs` produz `ValueFlow` conservador entre funções, distinguindo oracle usado só em guarda de oracle passado à conversão. Testes cobrem oito fixtures; CLI executada com solc local 0.8.37. `cargo test -p security-engine --offline`: 22 testes passaram; `git diff --check` limpo.
- M2 resolveu herança no mesmo arquivo e `ProxyRelation` para alvo `immutable` criado localmente. Endereço de implantação ou campo mutável continua incerto. Próxima etapa: M3, bytecode EVM. Loops têm aproximação de uma iteração; chamadas desconhecidas não propagam retorno presumido. `try`, assembly, ternários múltiplos e modifiers com zero/múltiplos `_` ainda não têm fluxo completo. Não afirme que CFG completo ou engenharia reversa de bytecode já funcionam.
- Clippy estrito aponta 13 avisos `collapsible_if`; Clippy com essa regra liberada e `-D warnings` passou. Limpeza de estilo fica separada do fechamento funcional do M2.
- Atualize este arquivo e `/agente.md` ao final de cada etapa, como solicitado pelo usuário.

## M3 — recorte legado concluído

- Leia `docs/BYTECODE-RECONSTRUCTION.md`. O módulo `bytecode.rs` produz `BytecodeAnalysis` separado da IR source, com normalização hex/CBOR, disassembly, blocos, saltos literais, dispatch direto, efeitos por PC, resumo por selector e incertezas explícitas. O CLI oferece `--bytecode` e `--compare-bytecode` com saída local do solc.
- `bytecode_flow.rs` propaga stack e palavras de memória por CFG conhecido; liga calldata, callvalue, storage e resultados de chamadas a storage, chamadas, retornos e condições, com PCs e limites explícitos. Saltos constantes propagados ampliam o CFG. A análise permanece genérica e não prova caminho factível.
- `BytecodeAnalysis.value_flows` mantém origens, sinks, PCs aritméticos e confiança. Os resumos por selector são `Tentative` porque blocos podem ser compartilhados. EOF, creation code, ABI, layout completo de storage, memória parcial e saltos ainda dinâmicos permanecem fora do recorte ou em `unresolved`.
- `compare_with_solc_contract` rotula operações econômicas pelo nome do método apenas com runtime normalizado idêntico. Selector match sem igualdade de runtime é `Tentative`; não confunda observação de opcode com vulnerabilidade.
- `cargo fmt --all` e `cargo check -p security-engine --offline` executados; nenhum teste foi adicionado ou executado nesta etapa. Próximo marco: M4, análise econômica determinística.
- A revisão `3d2a9a6` também foi enviada ao VPS e compilada em release com Rust 1.93.1. A documentação operacional em `docs/VPS-SETUP.md` foi atualizada para os novos modos do CLI.

## Estratégia de ferramentas após M2

- Leia `docs/TOOLING-STRATEGY.md` antes de integrar scanner, fuzzer ou provador. A análise comparativa foi documentada, sem instalação ou execução dessas ferramentas em alvos.
- Comece por Slither + IR Rust e invariantes Foundry. Use Echidna ou Medusa somente com ganho medido; Halmos é o primeiro piloto simbólico. Mythril é seletivo após M3. Manticore foi arquivado e Securify2 não compõe o fluxo principal.
- Não envie source privado a Certora ou outro serviço externo sem autorização específica; o CLI padrão de Certora submete jobs ao servidor. Registre resultados, versões, premissas, cobertura, contraexemplos e custo de IA, sem inventar percentuais de economia.

## Infraestrutura informada

- O VPS Hostinger KVM 4 foi configurado em 2026-10-18. Leia `docs/VPS-SETUP.md` antes de novas alterações: Ubuntu 26.04.1, 4 vCPU, 16 GiB RAM, disco de 200 GB, SSH funcional, UFW somente OpenSSH, Fail2ban, swap de 16 GB e usuário `toupera` sem sudo.
- Reinicialização após atualizações concluída e acesso SSH, proteção, swap e ferramentas reconfirmados; sem marcador de reinicialização pendente.
- Rust/Cargo 1.93.1, solc 0.8.37, Slither 0.11.6 e Foundry 1.8.5 estão instalados. O snapshot privado está em `/srv/toupera-agent`; o CLI release foi compilado e executado na fixture `VaultCallFlowFixture.sol`. Slither também analisou essa fixture. Ver evidência e limites no documento de VPS.
- DeepSeek será via API, mas não há integração, chave, serviço web ou endpoint implantado. Não alegue que o agente autônomo completo está rodando. Não grave segredos no repositório ou servidor sem mecanismo apropriado.
- O login root e por senha existente não foi desabilitado, para evitar perda de acesso. Alterações futuras de SSH exigem manter uma via administrativa verificada.
- Manter fuzzing com paralelismo limitado até medir consumo. Não armazenar chaves SSH nem chaves da API no repositório.
- Os 10 executores pedidos foram instalados: Slither, Mythril, Securify2, Manticore, Foundry, Echidna, Medusa, Certora CLI, Halmos e Kontrol. Alguns usam Python isolado ou imagens Podman rootless; confirme versões, caminhos, limites e invocação em `docs/VPS-SETUP.md`. Instalação não é integração automática. Certora não possui chave e não deve receber source privado sem autorização específica. Securify2/Manticore são legados.

## Complemento de engenharia reversa após M3

- `bytecode_evmole.rs` chama EVMole 0.9.4 localmente sobre runtime normalizado e mantém `BytecodeAnalysis.evmole` separado dos fatos nativos. Relatório contém candidatos de selectors/ABI, storage e CFG com PCs quando a ferramenta os fornece, `Tentative` e divergências. Não promova seus resultados automaticamente a `ControlEdge` ou `ValueFlow` nativos.
- A comparação local com solc aceita selector ABI exclusivo do EVMole, mas `BytecodeMethodMatch.recovered_by` expõe a origem e sua confiança permanece `Tentative`. Rótulos econômicos por nome exigem runtime compilado idêntico. Layout de storage sem PC confirmado continua candidato.
- O companion é omitido acima de 24.576 bytes e preserva resultado nativo se falhar. Mythril instalado continua sem orquestração pelo Rust. Leia `docs/BYTECODE-RECONSTRUCTION.md` para o contrato de saída.
- Compilação local com `cargo check -p security-engine --offline`; nenhum teste automatizado foi executado nesta etapa. Atualize `agente.md` e este arquivo depois de cada etapa importante, conforme pedido do usuário.

## Heimdall e Gigahorse após M3

- `bytecode_companions.rs` importa ABI Heimdall e TAC Gigahorse em relatório separado. `--bytecode-companions` exige runtime, `abi.json` e `contract.tac`; o wrapper `scripts/run-bytecode-companions.sh` executa os dois CLIs localmente e conserva os outputs. Heimdall usa `--skip-resolving`; Gigahorse usa Podman sem rede.
- Assinaturas/ABI e CFG propostos são `Tentative`. Correlacione selectors e PCs com o decoder nativo/EVMole; não incorpore automaticamente arestas TAC ou expressões ao `ValueFlow` nativo. Importação não confirma vulnerabilidade. Consulte `docs/BYTECODE-RECONSTRUCTION.md`.
- Após esta etapa, `cargo fmt --all`, `cargo check --locked --offline -p security-engine` e `git diff --check` passaram; testes automatizados não foram adicionados ou executados.
- No VPS, Heimdall está em `/usr/local/bin/heimdall` (asset release 0.9.3; `--version` informa 0.9.2) e Gigahorse está em imagem Podman fixada por digest; veja `docs/VPS-SETUP.md`. Não afirmar que decompilação de contrato foi executada só porque as ferramentas foram instaladas.
- A revisão de implementação `a3b4d6f` foi publicada e compilada em release no VPS; `.deployed-commit` deve acompanhar a revisão corrente. O fluxo completo Heimdall→Gigahorse→importação ainda não foi executado em fixture ou alvo.

## M4 — análise econômica source-backed

- `economic_analysis.rs` produz candidatos `UNVERIFIED` com evidência por source span, regras e premissas. `--economic-candidates <source.sol> [solc] [slither.json]` expõe o relatório. O wrapper `scripts/run-economic-candidates.sh` roda Slither localmente e conserva JSON e versões. Leia `docs/ECONOMIC-ANALYSIS.md` antes de alterar regras.
- Deduplicação usa contrato, função e categoria. JSON Slither precisa de `success=true`; detectores sem mapeamento, função inequívoca ou span válido não viram candidato. Nunca trate `impact`/`confidence` do Slither ou uma regra Rust como confirmação econômica.
- A pedido do usuário, não executar testes ou análise de fixtures até a implementação geral estar completa. Compilação, formatação e sintaxe do shell foram conferidas; calibração de falsos positivos, cobertura e economia de IA permanece pendente. Atualize `agente.md` e este arquivo após etapas futuras.
- A revisão de implementação M4 `d2ed831` foi publicada e compilada em release no VPS. O marcador `.deployed-commit` deve acompanhar o HEAD sincronizado. Não afirmar que o wrapper Slither ou os detectores foram executados em fixture.

## M5 — deduplicação e contexto mínimo

- `context_package.rs` usa SHA-256 de source, AST, função, compiler, engine e entradas opcionais Slither/runtime normalizado. `--context-package` emite JSON local e usa cache privado `0700`/`0600`; acertos pulam solc, reconstrução e M4. O runtime opcional é fingerprint apenas, sem atribuição de identidade ao source.
- Cada slice contém trechos limitados de função, relações e evidências, storage, aritmética, oracles, chamadas e perguntas de invariantes; `truncated` sinaliza corte. O pacote conserva `UNVERIFIED` e incertezas. Leia `docs/CONTEXT-PACKAGE.md` antes de M6.
- M6 deve consumir apenas slices necessários; não envie repositório inteiro nem source privado a um serviço externo automaticamente. Não há integração DeepSeek ou segredo implantado. A calibração, fixtures e benchmarks seguem adiados por instrução do usuário até implementação completa.
- `cargo fmt --all`, `cargo check --locked --offline -p security-engine` e `git diff --check` foram usados durante M5. Nenhum teste ou fixture foi executado. A revisão de código `acd4d48` foi publicada, sincronizada e compilada em release no VPS; `.deployed-commit` registrou a revisão antes do complemento documental final. Atualize este arquivo e `agente.md` após cada etapa.

## M6 — revisão LLM estruturada

- `llm_review.rs` separa seleção/prompt do fornecedor com `HypothesisProvider`; `DeepSeekProvider` usa HTTPS para Chat Completions e chave apenas do ambiente. `--llm-plan` é offline; `--llm-review` exige `--allow-external`, modelo/fornecedor e diretório de cache privado. Não rodar contra source privado sem escopo que permita envio externo.
- O payload da API é lista permitida de campos de um slice M5, não o pacote completo ou repositório. Gate: candidato `UNVERIFIED` com evidência e perguntas; dedup por contrato, função, categoria e hash da função. O parser exige JSON com disposição e, se hipótese, premissas, caminhos e teste Foundry sugerido. Nenhuma saída do modelo altera o estado além de `UNVERIFIED`.
- `CachingProvider` reaproveita apenas respostas válidas; relatório separa chamadas/tokens novos de cache local e aceita rate card opcional para custo estimado. Leia `docs/LLM-REVIEW.md` e `docs/ROADMAP-CHECKLIST.md` antes de M7. Nenhuma chave foi instalada, chamada API/fixture/teste executado ou economia medida; compilação Rust 1.93 e formatação passaram. Revisão de código `6ff127a` publicada, sincronizada e compilada em release no VPS; `.deployed-commit` registrou a revisão antes do complemento documental final. O diretório `web/` não rastreado deve ser tratado como artefato de outro fluxo e não foi incluído no commit/deploy M6.

## M7 — juiz Foundry local

- `foundry_judge.rs` prepara um caso privado por hipótese `UNVERIFIED` de M6, confere chave do candidato e SHA-256 do source M5, e guarda modelo/assunções/teste sugerido só em JSON. O scaffold Solidity falha intencionalmente; não converta texto da IA em código executável. Leia `docs/FOUNDRY-JUDGE.md`.
- `--foundry-run` só aceita harness específico alterado, revisão humana declarada, chamadas válidas e limites cobertos, source e configuração originais. Executa duas vezes `forge test --offline` com seed fixa, fuzz e JSON/traces privados, e classifica apenas `REPRODUCED_PENDING_REVIEW` ou `INCONCLUSIVE`. Sucesso de teste não valida asserções nem confirma vulnerabilidade. `--offline` não substitui isolamento de rede; revisar o harness e ambiente antes de executar.
- A pedido do usuário, não executar testes/fixtures/benchmarks até a implementação geral estar pronta. M7 foi apenas compilado e formatado; cobertura, taxas de chamadas/reverts, fuzzers adicionais e Halmos seguem pendentes. Prosseguir em M8; atualizar `agente.md` e este arquivo ao fim da etapa. Preservar `web/` não rastreado.
- A revisão M7 `b945a96` foi publicada no GitHub privado, sincronizada e compilada em release no VPS sob `toupera`. O marcador `.deployed-commit` registrou essa revisão antes do complemento documental. Nenhum harness, teste Foundry, fixture ou modelo foi executado no VPS nesta etapa.

## M8 — pacote de evidência

- `evidence_package.rs` prepara um snapshot privado do caso M7, com SHA-256 de cada artefato. Sem duas execuções positivas, o manifesto fica `PENDING_REPRODUCTION`; se ambas forem consistentes, `PENDING_HUMAN_REVIEW`. `--evidence-plan` não executa Foundry, IA ou análise de alvo.
- `--evidence-finalize` valida hashes, identidade do caso, status diretamente nos JSON Foundry e revisão humana preenchida sobre escopo, asserções, impacto, duplicidade, ambiente e passos. Só então grava `report.md`/`ready.json` em `REPORT_READY`; não envia automaticamente o relatório. Os campos humanos são atestados que precisam ser conferidos de verdade. Leia `docs/EVIDENCE-DELIVERY.md`.
- Testes, fixtures, métricas e relatórios reais permanecem adiados por instrução do usuário até implementação geral completa. Continuar recursos de instrumentação M7 antes da validação. Atualizar ambos os arquivos de agente após a próxima etapa. Preservar `web/` não rastreado.
- A revisão M8 `ea48456` foi publicada no GitHub privado, sincronizada e compilada em release no VPS sob `toupera`; `.deployed-commit` registrou a revisão antes do complemento documental. Nenhum pacote M8, relatório, fixture ou teste Foundry foi executado.

## Complemento M7 — contadores e cobertura

- `foundry_judge.rs` usa os campos `kind.Fuzz.runs` e, quando existem invariantes, `kind.Invariant.calls/reverts` do JSON Foundry 1.8.5. O primeiro é gate de 256 entradas em cada uma das duas rodadas; os últimos contam chamadas de handlers e não devem ser descritos como chamadas econômicas válidas ao alvo.
- Após testes positivos, executa `forge coverage --report lcov` com a mesma seed e exige ao menos uma linha de `src/Target.sol` atingida. M8 copia LCOV/log e reconfere cobertura e fuzz diretamente dos arquivos antes de `PENDING_HUMAN_REVIEW` ou `REPORT_READY`. Inconclusivos permanecem pendentes; nenhuma hipótese é rejeitada automaticamente.
- Nenhum teste, fixture, comando de cobertura ou benchmark foi executado nesta etapa, conforme pedido do usuário. Apenas formatação, compilação Rust 1.93 e diff estático. Leia `docs/FOUNDRY-JUDGE.md`; atualizar ambos os arquivos de agente após próximas etapas. Preservar `web/` não rastreado.
- Revisão `d47a909` publicada no GitHub privado, sincronizada e compilada em release no VPS sob `toupera`. `.deployed-commit` recebeu a revisão antes deste complemento documental; nenhuma campanha ou pacote real foi gerado.

## Fechamento M7 local — piloto sintético

- Leia `docs/M7-PILOT.md`, `docs/FOUNDRY-JUDGE.md` e o checklist antes de avançar. O juiz executou duas rodadas Foundry por ramo da fixture própria, com seed `0x5eed`, 256 fuzz runs por rodada, teste determinístico de assets 1..7 e LCOV. Ramo defeituoso: `REPRODUCED_PENDING_REVIEW`, preview 2 versus execução 3 em assets=1. Ramo corrigido: reprodução falha mas fuzz e domínio válido passam; `--foundry-reject` registrou `REJECTED` com revisão limitada à fixture sintética.
- Rejeição exige `rejection-review.json` completo, JSON/traces de duas rodadas, target coberto e hashes consistentes. M8 transporta a rejeição como `REJECTED`, nunca `REPORT_READY`. Campos de revisão são atestados declarados, não prova automática de revisão humana.
- Halmos encontrou contraexemplo assets=1 no harness simbólico baseado em herança. Não executar Echidna/Medusa sem ganho esperado de alcance ou achados únicos; Kontrol/Certora apenas para propriedade crítica com premissas explícitas. O comando `--context-package` seguido de `--llm-plan` não selecionou a fixture: há lacuna M4, e o piloto M7 usou casos montados manualmente. Não apresentar o fluxo M4→M7 como validado de ponta a ponta.
- Artefatos brutos privados estão em `/srv/toupera-agent/private-output/m7-pilot-v2/`. Não copiar para GitHub. O diretório local `web/` é de outro fluxo e não deve ser adicionado. Atualizar este arquivo e `agente.md` após novas etapas.
- Pacotes M8 do piloto: positivo `PENDING_HUMAN_REVIEW` (11 artefatos), negativo `REJECTED` (13). `--evidence-finalize` recusou o rejeitado como esperado. Não há `REPORT_READY` ou revisão humana de relatório.

## Piloto EVM real — YieldVault Sepolia

- Documento principal: `docs/REAL-EVM-PILOT.md`. Alvo público Sepolia `0xa32f9d514804084839b59972F8b43e616BB4E32b`, bloco 11944548. O bundle verificado tem 19 fontes; recompilação com solc 0.8.24 / `cancun` produziu runtime de 9.229 bytes. As 21 diferenças onchain são apenas duas faixas `immutable` informadas pelo solc; a visão `forge flatten` é só para M2.
- M3 agora distingue igualdade direta de igualdade mascarando somente `immutableReferences` e registra os bytes onchain. M5 aceita `--evm-version` em compilação, pacote e chave de cache. M6 aceita lote sem candidatos localmente, com `provider=none` e zero chamadas. Teste novo garante que uma divergência fora da faixa `immutable` continua falhando.
- Fluxo observado: M1/M2 54 funções; M3 31 selectors correlacionados e 299 incertezas; M4 zero candidatos, embora Slither tenha importado 51 sinais (zero mapeados); M5 `cache_hit=true` na repetição; M6 zero requests; M7 zero casos; M8 sem caso/relatório. O alerta de igualdade estrita em `maxMint` usa `uint256.max` como sentinela de cap ilimitado, sem prova de falha. Não inferir ausência de bugs.
- O ramo automático com candidato M4→M8 continua aberto; a fixture M7 positiva foi montada manualmente. Artefatos brutos ficam em `private-output/real-targets/yield-vault/`, ignorado pelo Git. Não houve transação, fork, DeepSeek nem exploração. `web/` não rastreado não pertence a esta etapa.
- Verificação: `cargo +1.93.0 fmt --all`, `cargo +1.93.0 test --locked --offline -p security-engine` (23 testes), binário debug e repetição dos comandos reais M3/M5/M6/M7. Atualizar `agente.md` e este arquivo a cada próxima etapa.
## Ligações da leitura por variável

- A lente registra índices de arrays/mappings como famílias simbólicas, argumentos→parâmetros e retornos→resultados para chamadas internas por identificador. Relações entre funções têm confiança `syntactic_call_summary`; chamada externa, assembly, aliasing e viabilidade seguem abertos.
- `verified_variables` mantém AST IDs, arquivos e offsets do bundle multifile verificado separados do source achatado. A referência em faixa do source map ainda é `tentative_source_range` para o predicado observado.
- `--guided-scenarios` valida a identidade da lente e do relatório forense, seleciona famílias por tipos/nomes, executa somente no Anvil local do VPS e registra sequência e decisões observadas. Grupos de `--local-coverage`: `guided_limit`, `guided_actor`, `guided_asset_state`. Consulte `docs/VARIABLE-LENS.md`.
- Nunca incluir `web/` não rastreado nem arquivos privados de `private-output/` no Git. Atualizar ambos os arquivos de agente e o checklist após cada etapa.
- VPS: lente SSA v2 gerou 742 variáveis, 741 relações totais, 274 resumos entre funções, oito acessos indexados; `guided-scenarios-v2` corrigido gerou 33 casos/29 concluídos/quatro reversões/zero falhas ou diferenças e nenhum resultado `JUMPI` novo contra v7. `cached-reassessment-guided-v1` preservou 29 zonas/19 quarentenas com zero RPC. O RPC principal não serviu estado histórico; o alternativo do manifesto serviu. A primeira rodada guiada usou `transfer` em ordem ABI errada e não é evidência. A assinatura nova de `--guided-scenarios` exige cobertura anterior para delta automático.
- `guided-scenarios-v3/report.json` confirma no próprio JSON 58 resultados anteriores, nenhum resultado/PC novo; serviço transitório terminou com status 0. Priorizar futuramente condições não observadas/um só sentido e viabilidade de estados. Não afirmar cobertura por função ou conclusão do scanner.

## Camada de ligações graduadas

- `variable_lens.rs` agora diferencia chaves fixas/simbólicas/não resolvidas, gera relações possíveis entre funções por estado persistente, registra chamadas externas e assembly como limites do grafo e atribui PCs somente quando o source map do bundle verificado cabe em uma única função/modifier. Relatório `source_coverage` usa denominador estático de origem única, nunca caminhos alcançáveis.
- `local_scenarios.rs` faz `eth_call` prévio no filho Anvil, compara recibo, captura slots `SLOAD`/`SSTORE` do frame raiz, verifica continuidade de sequência e fórmula de mapping por endereço nas bases 0..63; coincidência não identifica automaticamente a variável de source.
- VPS: `variable-lens-ssa-v7/report.json` tem 12 pares de chave, 11 ligações possíveis de estado, seis limites de chamada, 36 assembly e 41/110 PCs estáticos atribuídos com origem única (69 lacunas). `guided-scenarios-v6/` tem 36 casos, 32 concluídos, quatro reversões, zero falhas/diferenças, quatro sequências contínuas com dois slots reutilizados cada, dois candidatos de mapping base 0 para atores distintos e zero resultado novo. `reassessment.json` manteve 29 zonas/19 quarentenas sem RPC. Tudo privado no VPS.
- Limites restantes: alias dinâmico fora dos valores executados, comportamento interno de contrato externo/assembly dinâmico, prova de alcançabilidade geral e denominador de ramos alcançáveis. Não apresentar esses itens como resolvidos. Atualizar ambos os arquivos de agente após etapa futura; preservar `web/`.

## Triagem local pré-IA (2026-10-19)

- `evidence_filter.rs` exige identidade/hashes de M4, lente, cenários, cobertura e M5. Pontua prioridades já calculadas pela lente, lista PCs de origem de source com menos de dois resultados e sugere famílias de cenários; ver `docs/EVIDENCE-FILTER.md`.
- `--llm-plan` e `--llm-review` exigem `--evidence-filter` vinculado ao hash exato do pacote M5. Só uma diferença concreta repetida em duas sequências distintas, com pré-checagem válida e sem falha RPC, pode liberar uma chave candidata; nenhuma pontuação isolada a libera. Zero elegíveis significa zero chamadas.
- O modo guiado agora varia até 16 valores locais e para por orçamento ou platô de oito casos; registra contagem e motivo. Continua pendente adaptar ordens/estados a PCs específicos em múltiplas rodadas. Preserve artefatos privados no VPS e `web/` fora de commits.

- VPS, rodada 2026-10-19: `toupera-evidence-feedback-20261019.service` terminou com status 0; 44 casos, 40 concluídos, quatro reversões esperadas, oito variações adicionais, parada por platô, zero decisões novas/erros RPC/diferenças. O filtro/planos privados mantiveram zero chaves/pedidos, sem IA. A ordenação final percorre até quatro relações de dependência a partir de usos econômicos. O próximo passo técnico é variar ordens e estados conforme lacunas específicas.

## Precisão v2 — seleção e leitura local (2026-10-19)

- A lente extrai `numeric_boundaries` de comparações diretas entre parâmetro e literal na AST de análise. O modo guiado varia abaixo/no/acima se couber no orçamento e, como fallback, localiza transições de `previewMint`/`previewRedeem` por consultas limitadas ao Anvil local. Os pontos são entradas candidatas; não há prova geral de monotonicidade ou equivalência ao runtime.
- O modo guiado acrescenta até oito sequências de ordem/estado isoladas por snapshot, com parada após quatro sem novo resultado no PC alvo. A rodada de valores mantém limite de 16 casos e parada por oito sem resultado no alvo. Somente Anvil em loopback recebe as transações locais.
- O filtro informa observações por função com denominador de PCs estáticos de origem única, candidatos de slot de mapping cruzados com layout, e alvos de chamada observados com mudanças de saldo concomitantes. Não atribuir causalidade de chamada, alias geral ou alcance a PC não observado. Artefatos privados ficam no VPS; `web/` fora dos commits.


Rodada `precision-v2` no VPS (2026-10-19): serviço `toupera-precision-transition-20261019.service` terminou com status 0. A lente listou 12 comparações diretas em funções auxiliares, nenhuma aplicável diretamente às quatro operações principais. A escolha por mudança de preview fez 68 consultas locais e originou seis casos; a rodada de valores parou após oito casos sem resultado novo no ponto alvo. Quatro sequências de ordem/estado foram executadas antes do próprio platô. Total: 50 casos, 46 concluídos, quatro reversões esperadas, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado permaneceu em 50/110 PCs estáticos, sem resultado novo; 41 PCs têm origem única de source, 23 deles observados. O filtro registrou 25 resumos por função, duas coincidências de slot/layout, quatro acessos indexados sem coincidência, um alvo de chamada observado e zero pedidos de IA. Todas as contagens se limitam ao estado e às entradas da rodada.

## Chip adaptativo de escolha local (2026-10-19)

- `adaptive_chip.rs` integra `--guided-scenarios`. O argumento opcional de relatório anterior só é aceito quando identidade, runtime, bloco e hash do cenário gerador da cobertura coincidem. O chip compara operação/valor e classe de comportamento, seleciona variações locais e registra motivo de parada; ver `docs/ADAPTIVE-CHIP.md`.
- A memória tem limite de 256 observações; a rodada de feedback tem limite de 16 casos e platô de oito sem nova classe ou resultado de ponto alvo. Não converter classe, prioridade ou PC não observado em alegação de diferença confirmada. Rodar cenários somente no VPS, manter `private-output/` privado e `web/` não rastreado fora do commit.

Rodada precision-chip-v1 no VPS: serviço toupera-adaptive-chip-20261019.service terminou com status 0. O chip carregou oito observações, evitou 20 entradas repetidas e quatro vizinhos de mesma classe, ordenou 61 opções e parou após oito casos de feedback sem classe ou resultado alvo novo. Foram 50 casos totais, 46 concluídos, quatro reversões, zero falhas RPC/diferenças, 50/110 PCs estáticos e 58 resultados observados sem aumento. O filtro ficou sem candidato e o plano de IA vazio. Dados brutos somente no VPS.

## Plano local e medição (2026-10-19)

- `forensic_plan.rs`: comando `--forensic-plan` valida cinco artefatos da mesma identidade; combina condições simples de AST/source map com classes de origem do bytecode, calcula valores de dois lados, prioriza pares de operações por slots observados e registra relações numéricas. O modo guiado aceita o plano opcional e recusa novo ciclo se ele indicar platô sem entrada distinta. `--source-benchmark` e `tools/measure_local.py` medem três fontes/cenários próprios no VPS. Leia `docs/SIMPLE-PLAN.md`.
- Serviço VPS `toupera-simple-plan-20261019.service` status 0: 52 casos, 48 concluídos, quatro reversões, zero falhas/diferenças e nenhum resultado/PC novo. Plano final: 17 condições simples, 36 PCs de origem única ainda não resolvidos por faixa estreita, 46/102 PCs com alguma classe abstrata de origem, 240 deltas, 16 pares de preview e 48 diferenças contábeis sem divergência observada. O comando recusou repetição por platô antes de RPC. Medição própria: Rounding 15→16, Modifier 9→12, Loop 13→11 resultados com menos chamadas; não esconder a perda no loop.
- Todos os JSONs de alvo e medição detalhada permanecem em `private-output/` no VPS. Publicar somente código/docs/resumo no Git privado. Não alegar resolução de condições compostas, alcance de PCs desconhecidos ou ganho geral. Preservar `web/` não rastreado.

## Painel wCOP
O formulário do site aceita somente wCOP na Ethereum. `tools/site_jobs.py` roda no VPS sob `toupera`, com API protegida no Caddy, três Markdown na raiz do projeto e histórico privado. Atualizar `agente.md`, este arquivo e o checklist após mudanças nesta etapa. Ver `docs/SITE-WCOP.md`.
O último trabalho wCOP está em `private-output/site-jobs/356a04b350144dc6/`. A fonte no Git não deve incluir credencial/hash ativo do Caddy; publicar apenas arquivos web permitidos, sem `deploy/`.
A execução final autenticada está em `private-output/site-jobs/b0a86a00d5664e97/`. O trabalhador lê `smart.md`; o status trata morte do processo e prazo prolongado.
O painel wCOP inclui `tools/wcop_readonly.t.sol` e roda três regras Foundry no fork local do bloco fixado. Os outros executores permanecem condicionados a propriedades/compatibilidade.
A última execução automática wCOP é `private-output/site-jobs/ba5146dd978d45e2/`: três ferramentas executadas, Foundry 3/3, relatório no painel.


## Etapa wCOP: histórico, peneira e sequências locais (2026-10-19)

- O painel continua limitado ao wCOP no Ethereum e grava os dados por trabalho em `private-output/site-jobs`.
- `tools/wcop_history.py` lê a criação e os eventos do endereço até o bloco fixado. Cada mudança traz bloco, transação e índice do evento. O relatório compara emissão menos retirada com `totalSupply` lido no mesmo bloco e marca divergência.
- `security-engine --wcop-sift` agrupa as dúvidas do bytecode e ordena até 30 pontos para revisão. As 372 ocorrências ligadas à versão da rede ficam separadas do restante; a ordem não representa chance de falha.
- `tools/wcop_readonly.t.sol` inclui ida e volta de saldo e uso de autorização, além das três regras anteriores, sempre em cópia local do bloco. Um titular com saldo é escolhido a partir dos eventos e confirmado por leitura no bloco.
- Slither e Foundry usam o binário Solidity 0.8.27 obtido da distribuição oficial com SHA-256 conferido. O binário fica no VPS e não entra no Git.
- O relatório deve distinguir consulta completa, leitura incompleta e resultado local. Não transformar sinal bruto em conclusão.
- Na implantação, conferir `history.json`, `sift.json`, `tools.json` e `end_smart_result.md` de um trabalho real antes de declarar a etapa concluída.


### Fechamento desta etapa (2026-10-19)

- Trabalho final do painel: `f36ae038ef634853`, completo em bloco fixado. 236 eventos foram classificados: 192 transferências, 27 autorizações, 11 concessões de papel, 4 revogações, 1 inicialização e 1 mudança de implementação.
- Emissão menos retirada conferiu com `totalSupply` no bloco. O resultado descreve consistência observada, sem afirmar que todos os caminhos do contrato foram cobertos.
- A peneira Rust recebeu 475 dúvidas, separou 372 dependentes da versão da rede e ordenou 30 pontos para revisão. O texto deixa explícito que essa ordem não é probabilidade de falha.
- Slither com solc 0.8.27 retornou saída estruturada e código 0; o único sinal no arquivo principal é informativo sobre pragma. Foundry concluiu 5/5 cenários locais. Mythril registrou um sinal bruto ainda sem confirmação.
- O JavaScript do painel estava inválido. O formulário e o terminal foram refeitos no VPS; o arquivo corrigido foi publicado em `/var/www/toupeiraagente/current`. Site e arquivo JS respondem 200; API sem credencial responde 401.
- Arquivos detalhados continuam privados no VPS. Código e documentos devem ser enviados ao repositório privado; não incluir o binário solc, credenciais, relatórios privados nem dados da pasta `private-output`.
