# Memória operacional — Toupera Agent

## Homepage pública — Toupeira Agente

- Pedido do usuário: criar e publicar uma homepage visual em `toupeiraagente.tech`, baseada na referência do robô branco/azul, com fumaça e animações. A marca pública deve ser **Toupeira Agente**, alinhada ao domínio.
- Implementação em `homepage/`, separada do `web/` não rastreado de outro fluxo. HTML/CSS/JS estáticos, fundo ImageGen derivado da referência, fonte local, fumaça Canvas, partículas, luz e etapas interativas. Não há formulário público conectado ao motor.
- Conferidos no navegador desktop/mobile: composição, abas, navegação por teclado e pausa dos efeitos. Preparada configuração Caddy/HTTPS para a publicação solicitada; registrar resultado de produção ao concluir. Ver `homepage/README.md` para assets, prompt e estrutura de publicação.
- Publicação concluída: `https://toupeiraagente.tech/` retorna 200 em IPv4/IPv6; HTTP redireciona por 308 e `www` por 301 ao domínio principal. Caddy 2.11.7 instalado pelo repositório oficial, ativo e habilitado, com certificados Let's Encrypt para ambos os hosts. UFW libera TCP 80/443 além do SSH. Raiz pública `/var/www/toupeiraagente/current`, release `04a1473`; somente arquivos da homepage. Artefatos privados retornam 404. Navegador de produção sem erros; imagem WebP 172.206 bytes e fonte local. Versão web em `/var/www/toupeiraagente/.deployed-commit`, separada do marcador do motor.

## Objetivo ativo — scanner forense automático em Rust (2026-10-18)

O usuário rejeitou tratar o mapa atual como auditoria forense completa. O motor deve produzir o contexto por conta própria, usando aritmética EVM de 256 bits, cronologia integral desde o deploy quando o RPC permitir, transações/recibos/rastros, evolução de estado, invariantes econômicas, simulação local e cobertura mensurável. O segundo motor de quarentena recebe divergências concretas; o primeiro refaz o mapa. IA só interpreta hipóteses ainda ambíguas depois dos filtros determinísticos.

Registrar progresso em `docs/ROADMAP-CHECKLIST.md` e neste arquivo a cada etapa. Não marcar como concluídos replay de execução de transações, rastros internos, fuzzing ou cobertura total enquanto não houver execução e evidência. Artefatos de alvo seguem privados. Preservar `web/` não rastreado. Instrução atual do usuário: executar simulações somente no VPS, em processo autônomo, e verificar se ele consegue; não simular no ambiente local desta sessão.

## Etapa atual — resumo de caminhos e pesquisa da cadeia de variáveis

- `--local-coverage` resume por operação, PC e grupo de entrada os rastros já existentes. `local-coverage-v2.json` foi gerado no VPS a partir de `local-scenarios-v7/report.json`, sem novo fork/RPC: 24 casos, 110 PCs `JUMPI` estáticos, 50 vistos, 58 resultados; quatro entradas zero já cobrem 56 resultados, 16 positivos comuns não acrescentam nenhum e dois casos acima dos limites acrescentam dois.
- A conclusão vale apenas para o estado e a grade observados. A correspondência PC→função Solidity, alcance dos 60 PCs não vistos e cobertura de estados ainda não foram medidos. Não tratar o resumo como aprovação do contrato.
- Nova proposta pesquisada em fontes primárias: [segunda leitura por variável](docs/VARIABLE-LENS.md). Inventariar cada declaração e seguir origem→transformação→guarda→escrita/efeito com AST/layout, SlithIR SSA e source maps; Gigahorse é apoio tentativo quando source não corresponde. O motor ainda não executa a cadeia completa. Próximo recorte: inventário com proveniência, depois ligação a PCs e geração de estados no VPS. Atualizar `codex/agent.md` após cada etapa.

## Etapa implementada — primeiro grafo por variável

- `--variable-lens` valida manifesto/relatórios/hashes e executa offline no VPS. `variable-lens-v4.json` registrou 742 declarações do source achatado, 180 relações sintáticas, 7 variáveis de estado persistente, 2 `immutable`; 50 PCs `JUMPI` observados, 18 faixas amplas sem atribuição e 17 faixas curtas com referências candidatas. Não houve novo fork, RPC ou IA.
- Exemplo de cadeia: `cap_`/`newCap` → `depositCap` → `cap` em `maxDeposit`, com spans e layout da compilação de análise (slot 6). Não atribuir esse slot ao runtime implantado sem validação específica. Fonte verificada e source achatado têm AST IDs diferentes; os PCs mantêm vínculo apenas tentativo por source map.
- Leia `docs/VARIABLE-LENS.md`. A etapa SSA está registrada abaixo; próximos gates: fluxo entre funções/chaves, ligação forte source verificado→runtime e cenários novos de estado/ator/ordem no VPS. Atualizar `codex/agent.md` e checklist após a próxima etapa; preservar `web/`.

## Complemento implementado — SlithIR SSA

- `scripts/export-variable-ssa.py` usa Slither 0.11.6 local para exportar operações SSA; `scripts/run-variable-lens.sh` encadeia exportação e Rust em diretório privado. O Rust confere SHA-256 do source, associa operandos posicionados a declarações por nome+offset e adiciona arestas `inferred_ssa` sem misturá-las às `direct_syntax`.
- Execução somente no VPS em `variable-lens-ssa-v1/`: 1.303 operações SlithIR, 287 relações SSA novas, 467 relações totais e 42 referências posicionadas sem correspondência exata. Arquivos `0600`, diretório `0700`. Sem fork, RPC, IA ou nova diferença econômica. Ainda faltam relações entre funções, chaves/índices e novos cenários de estado; ver `docs/VARIABLE-LENS.md`.

## Etapa mais recente — cronologia forense automática (2026-10-18)

- `forensic_probe.rs` usa `alloy-primitives::U256`, busca logs desde o deploy quando o RPC fornece arquivo, seleciona endpoint pela profundidade histórica depois de confirmar identidade, agrega mint/burn e transferências do asset, lê transações/recibos, tenta rastros internos, confere snapshots de estado nos blocos com eventos, propaga divergências à quarentena e emite métricas `coverage`.
- Na leitura já realizada de YieldVault Sepolia: 818.846 blocos inclusivos desde o deploy, 5 eventos do vault, 2 transferências do asset, 3 recibos e 1 bloco histórico com eventos verificado; 108 chamadas de borda, 103 retornaram. Não houve divergência observada nas relações conferidas. O RPC negou `debug_traceTransaction` com HTTP 429; `history_reconciled_trace_gaps` indica essa lacuna. Há 299 itens de bytecode não resolvidos. Isso não comprova segurança nem cobertura de caminhos.
- Uma tentativa manual de replay anterior ao pedido mais recente terminou por timeout e não forneceu evidência. Não executar novas simulações nesta etapa. Sequências em fork, cobertura de ramos, M4 positivo e reprodução M7/M8 continuam pendentes. Formatação e compilação foram verificadas; testes não foram rodados nesta atualização.
- Revisão de código `67de987` publicada no Git privado e compilada em release no VPS sob `toupera`; nenhum comando de simulação executado durante a implantação.
- Após nova autorização do usuário, `scripts/run-forensic-replay.sh` foi preparado para replay de até três transações históricas no VPS, com identidade fixa, timeout, rastros e resumo privados. O resultado consta da etapa de replay abaixo; não equiparar replay ao fechamento do scanner.

## Etapa mais recente — replay autônomo no VPS (2026-10-18 local)

- Job transitório `toupera-forensic-replay-20261018.service` executado como `toupera` no VPS, com limites de 8 GiB RAM, 200% CPU e 900 s. O script validou chain ID, hash do bloco e SHA-256 do runtime e rodou `cast run --no-bal` nas três transações históricas selecionadas.
- Resultado: 3/3 rastros com `Transaction successfully executed`, zero timeout/falha; serviço terminou `Result=success`. Deploy, depósito e distribuição de yield constam dos rastros. Evidência só em `/srv/toupera-agent/private-output/real-targets/yield-vault/replay-vps-20261018/`. Nenhuma transação foi transmitida à rede. Não foram medidas sequências inéditas em fork ou cobertura de ramos; esse gate permanece aberto.

## Etapa atual — cenários locais em Rust (2026-10-18 local)

- `--local-scenarios` abre Anvil filho em loopback após conferir chain/bloco/runtime/asset contra relatório base. O motor Rust executa deposit/mint/withdraw/redeem em snapshots isolados e depósito→resgate em sequência, com saldos e previews antes/depois, recibos e pontos `JUMPI` observados. Apenas o Anvil local recebe transações; o RPC de origem serve leitura de estado.
- O código compilou localmente; execução no VPS e interpretação de resultados ainda pendentes neste registro. `docs/LOCAL-SCENARIOS.md` descreve o formato e os limites. A contagem de `(depth,PC)` mistura vault e asset e não constitui percentual de cobertura. Diferenças observadas precisam ser vinculadas à quarentena antes de alimentar M5.
- Primeira execução no VPS: 18 cenários concluídos, zero reversões/falhas de RPC, 72 posições `(depth,PC)` com `JUMPI` no conjunto das chamadas; depósito→resgate devolveu 1 unidade mínima a menos por arredondamento. O comparador marcou falsamente `-0` versus `0` em dois resgates com preview zero; corrigido em código, repetição no VPS pendente. O manifesto agora pode carregar o relatório local, vinculá-lo à quarentena como pedido de revisão e refazer o mapa, sem promoção automática a M5.
- Segunda execução confirmou 18/18 cenários, zero reversões, zero diferenças de estado e 72 posições `JUMPI`; round trip `-1`. A reavaliação do mapa aceitou o hash e a identidade do relatório local, mas o RPC histórico retornou HTTP 429 antes de percorrer os logs, então o resultado dessa passagem ficou `partial` e não substitui a leitura completa anterior. Adicionado backoff limitado para 429 e espaçamento de chamadas HTTPS; repetir reavaliação no VPS.
- A repetição com espera para HTTP 429 completou novamente 818.846 blocos de logs, 3 recibos, 1 bloco de estado, 105/108 chamadas de borda e anexou 18 cenários; 29 zonas e 19 quarentenas foram reavaliadas sem mudança de prioridade ou candidato. `history_reconciled_trace_gaps` ainda separa rastros históricos do RPC dos rastros de cenário local.
- A etapa seguinte no código registra resultados `taken`/`not_taken` de `JUMPI` observados na stack local e adiciona `--cached-reassessment`, que reavalia todos os hotspots salvos somente se o relatório local apontar para o SHA-256 exato de uma coleta histórica completa. Essas duas alterações ainda precisam ser executadas no VPS neste registro.
- Primeira execução da nova medição completou 18/18 cenários no VPS, mas só decodificou 12 resultados para 72 posições `JUMPI`: Anvil inclui palavras hex de comprimento ímpar, que o parser recusava. O parser foi corrigido com preenchimento à esquerda; repetir antes de usar o número de lados percorridos. Nenhuma diferença econômica apareceu; round trip `-1`.
- Execução final no VPS após a correção: 18/18 operações, zero reversões/falhas/diferenças, 72 posições `JUMPI`, 80 resultados `taken`/`not_taken`, 8 posições vistas nos dois sentidos; round trip `-1` unidade mínima. Métrica agregada vault+token, sem denominador de cobertura. `--cached-reassessment` validou SHA-256 exato do relatório base e identidade, preservou 818.846 blocos e reavaliou 29 zonas/19 quarentenas com zero chamadas ao RPC e zero mudança de prioridade. Evidências privadas em `local-scenarios-v4/` e `cached-reassessment-v1/` no VPS.
- Em implementação: separar PCs `JUMPI` do frame raiz (`depth=1`) e contar `JUMPI` no runtime verificado para um denominador estático por contrato. A medida ainda não foi conferida no VPS. Mesmo após conferência, ela não provará alcance de caminhos nem cobertura por função; manter esses limites explícitos.
- Medida conferida no VPS: 110 `JUMPI` no runtime do vault, 50 PCs vistos no frame raiz e 56 resultados observados (6 PCs nos dois sentidos); a medida agregada vault+token permanece 72 PCs/80 resultados/8 nos dois sentidos. São contagens estáticas/observadas, não percentual de cobertura alcançável. O cache preservou 818.846 blocos e importou essas métricas com zero chamadas ao RPC. Nova grade de zero e acima de `maxWithdraw`/`maxRedeem` implementada; executar no VPS antes de reportar reversões.
- Primeira execução da grade ampliada: 24 cenários, 22 concluídos, 2 reversões nos valores uma unidade acima de `maxWithdraw`/`maxRedeem`, zero falhas de RPC. Dois avisos em `deposit(0)`/`mint(0)` eram formatação `-0` para saldo do ator; função comum de delta negativo corrigida, repetir antes de afirmar zero diferenças. Métrica do vault: 110 PCs estáticos, 50 vistos, 58 resultados e 8 PCs em ambos os sentidos.
- Repetição final `local-scenarios-v7`: 24 cenários, 22 concluídos, 2 reversões de limite, zero falhas de RPC e zero diferenças. `cached-reassessment-v3` vinculou o hash do relatório, preservou 29 zonas/19 quarentenas, sem mudança de prioridade ou RPC. Vault: 110 PCs estáticos, 50 vistos, 58 resultados, 8 PCs nos dois sentidos. Evidência privada apenas no VPS.
- Ajuste M4 em andamento: reconhecer a forma literal `x / d` versus `(x + d - 1) / d` com numerador/denominador iguais, emitindo só candidata `UNVERIFIED` com spans. Ainda conferir a seleção na fixture M7 no VPS; não declarar M4→M8 validado antes dessa passagem.
- Calibração M4 conferida no VPS: `RoundingVaultM7Fixture.sol` gerou 1 candidata `RoundingAsymmetry`/`UNVERIFIED` para `withdraw(uint256)` com 2 spans. M5 gerou 1 slice e `--llm-plan` selecionou 1 pedido offline. A condição `faultyPreview` ainda exige avaliação por instância; nenhuma API foi chamada ou novo caso M7 criado. Próximo gate é vincular candidato→revisão→caso→pacote sem fabricar decisão da IA nem revisão humana.

## Etapa mais recente — mapa forense e quarentena (2026-10-18)

- Pedido do usuário: o **código Rust** deve refazer o mapa de pontos fracos, isolar variável/caminho questionável em um segundo motor para auditoria mais profunda e devolver o resultado ao primeiro motor para reavaliação completa. Implementado em `crates/security-engine/src/forensic_probe.rs`, CLI `--forensic-probe`, integração M5 `--forensic` e documentação `docs/FORENSIC-QUARANTINE.md`.
- A primeira passagem pontuou 29 zonas, abriu 19 casos e fez 78 consultas de borda. A janela de 20.000 blocos era parcial; a etapa histórica acima ampliou a leitura desde o deploy. Motor 1 refez o mapa, sem anomalia observada nas relações conferidas; M5/M6 mantiveram zero candidatos/pedidos. Não inferir segurança.
- Identidade fixa: chain 11155111, bloco 11944548, hash `0x069c69928bac2b458e7e253512ae23919065b6fb8c94eb2a4b8f66864ef5d46a`, runtime SHA-256 `ee2630d13cdc5b0f2e8b12f512cdafff283015058f10a96322662441a3cff1f7`. Fallback RPC só depois de verificar chain, bloco e bytecode; source achatado não equivale ao bundle implantado, que foi recompilado com duas faixas `immutable`.
- Unitários passaram (27 testes) e execução real do novo CLI e M5/M6 concluíram. O ramo `anomaly_observed` → M5 tem teste sintético de vínculo de identidade, mas precisa de validação real com hipótese, harness e revisão antes de alegar pipeline positivo completo. Preservar `web/` não rastreado e artefatos brutos em `private-output/`.

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source e runtime EVM legado explícito já têm recortes de reconstrução separados. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0–M3 concluídos nos recortes documentados. M4 implementa triagem econômica source-backed, mas não selecionou a fórmula sintética de arredondamento do piloto M7; calibração segue pendente. M5 produz slices locais limitados e cache por conteúdo. M6 prepara requisições reduzidas e inclui adaptador DeepSeek opt-in, ainda sem chave ou chamada real. M7 local foi implementado e exercitado em fixture própria: reprodução, fuzz, cobertura, rejeição revisada e contraexemplo Halmos. M8 prepara pacote privado e gate de entrega, sem relatório real. Um contrato público real da Sepolia foi analisado somente por leitura e execução local; o fluxo parou sem candidato em M4.

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
Corrigir a lacuna M4 para arredondamento e calibrar M4–M6 em fixtures antes de auditar alvos reais. O M7 local está fechado no recorte sintético; cada alvo real ainda exige autorização, harness e revisão próprios. Não marcar achados como confirmados antes de reprodução e revisão. Echidna/Medusa só entram com lacuna de alcance ou achado único; Kontrol/Certora exigem propriedade crítica e premissas explícitas.

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
Calibração M4–M6, validação M7 por alvo real e entrega M8 permanecem pendentes. O piloto M7 sintético já foi executado; ver seção final e `docs/M7-PILOT.md`. Nunca armazenar secrets ou executar exploração em produção.

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

## Fechamento M7 local — piloto sintético (2026-10-18)
- `foundry_judge.rs` agora gera `rejection-review.json` e oferece `--foundry-reject`. O gate revalida duas execuções, JSON, statuses, fuzz >=256, LCOV do alvo, traces, hashes e atestado de revisão antes de registrar `runs/rejection.json`; uma execução falha isolada continua `INCONCLUSIVE`. `evidence_package.rs` preserva rejeição íntegra como `REJECTED` sem liberar `REPORT_READY`.
- A fixture própria `RoundingVaultM7Fixture.sol` alterna preview com arredondamento para baixo/cima, mantendo fee de 100 bps e execução para cima. Dois harnesses cobrem assets válidos 1..7 e 256 entradas de fuzz por rodada. No ramo defeituoso, `previewWithdraw(1)=2` e `withdraw(1)=3`, duas rodadas com três testes passando; LCOV 16/18 linhas, 3/5 branches. No corrigido, ambos retornam 3, `testReproduce()` falha nas duas rodadas, os outros testes passam; LCOV 17/18 linhas, 2/5 branches. Rejeição registrada somente para essa hipótese sintética com revisão identificada como Codex, não auditoria humana.
- Halmos encontrou `assets=1` como contraexemplo em propriedade curta herdada do alvo, oito caminhos em 0,19 s. Teste simbólico que criava contrato via `new` falhou por cheat code `deployCode` não suportado; substituído por herança. Echidna/Medusa não acrescentam alcance no domínio fixo do piloto; Kontrol/Certora não se justificam. Detalhes em `docs/M7-PILOT.md`.
- O CLI M5 analisou a fixture, mas `--llm-plan` devolveu zero candidatos. Casos M7 do piloto foram construídos manualmente em `private-output` e não validam seleção M4/M5/M6. Corrigir essa lacuna antes de afirmar fluxo completo. Nenhum alvo externo, API DeepSeek ou relatório real foi executado. `web/` não rastreado continua fora do trabalho.
- `--evidence-plan` no piloto produziu `PENDING_HUMAN_REVIEW` (11 artefatos) para o caso positivo e `REJECTED` (13) para o negativo. `--evidence-finalize` recusou o pacote rejeitado; nenhum `REPORT_READY` foi gerado.

## Piloto EVM real — YieldVault Sepolia (2026-10-18)

- Leia `docs/REAL-EVM-PILOT.md` antes de alterar o fluxo. Alvo público `0xa32f9d514804084839b59972F8b43e616BB4E32b`, chain ID 11155111, bloco 11944548. Source verificado multifile (19 arquivos) recompilado com solc 0.8.24, EVM `cancun`; runtime de 9.229 bytes. As 21 diferenças contra `eth_getCode` ficam somente nas duas faixas `immutableReferences` do solc. O arquivo `flattened.sol` de 3.966 linhas foi uma transformação para M2, não o source de identidade.
- `--compare-bytecode` agora registra `immutable_masked_runtime_match` e os valores implantados, preservando `normalized_runtime_match=false` para este deploy. O M5 aceita `--evm-version`, inclui a configuração na chave do cache e a registra no pacote. Um lote M6 vazio emite relatório local `provider=none` sem chave/API.
- M1/M2 registraram 54 funções; M3, 5.043 instruções, 611 blocos, 31 selectors e 299 incertezas; 31 métodos comparados. M4: zero sinais Rust/candidatos; Slither importou 51, mapeou zero. O alerta `incorrect-equality` em `maxMint` usa a sentinela explícita de cap ilimitado e não prova bug. M5 reuso idêntico `cache_hit=true`; M6 zero requests/chamadas; M7 zero casos; M8 sem entrada/relatório. Não alegar segurança ou pipeline positivo completo.
- O ramo positivo M4→M8 continua pendente de calibração em fixture conhecida e de alvo autorizado com hipótese elegível. O piloto sintético M7 anterior usou casos manuais. Artefatos brutos do alvo real estão em `private-output/real-targets/yield-vault/`, ignorado pelo Git; `web/` não rastreado segue fora do escopo. Não houve transação, fork, DeepSeek nem exploração.
- Nesta etapa: `cargo +1.93.0 fmt --all`, `cargo +1.93.0 test --locked --offline -p security-engine` (23 testes passaram), recompilação debug e comandos M3/M5/M6/M7 reais; cache repetido acertou. Após commit/deploy, registrar SHA e estado do VPS. Atualizar também `codex/agent.md` ao final de cada etapa futura.
## Etapa: ligações da cadeia de variáveis

- `variable_lens.rs` conserva acessos indexados como famílias simbólicas com base, chave, uso e origem; liga chamadas por identificador a parâmetros e resultados de retornos com confiança sintática. Não tratar essas relações como caminhos viáveis ou resolução de aliasing.
- O relatório agora preserva um inventário da AST multifile verificada, separado da AST achatada. PCs observados só recebem referências da faixa do source map verificado, ainda com confiança tentativa quanto ao controle do desvio.
- `--guided-scenarios` usa identidade/hash do relatório forense e da lente, seleciona limites, segundo ator e alteração de saldo do asset, e executa transações apenas no Anvil filho local. `--local-coverage` separa os grupos guiados. Leia `docs/VARIABLE-LENS.md` antes de alterar essa etapa.
- Manter `web/` não rastreado fora de commits e deploys. Executar cenários reais apenas no VPS; não copiar artefatos privados para Git. Atualizar este arquivo, `codex/agent.md` e o checklist após cada etapa.
- A rodada guiada corrigida no VPS usou o RPC histórico alternativo do manifesto após o principal não fornecer estado do bloco fixado. `guided-scenarios-v2` tem 33 casos (29 concluídos, quatro reversões, zero RPC/diferenças), nove deles guiados, com zero resultado `JUMPI` novo frente à grade v7. A preparação de segundo ator e doação exigiu corrigir a ordem ABI `transfer(address,uint256)`; a primeira rodada com essa ordem errada não é evidência válida. `cached-reassessment-guided-v1` manteve 29 zonas/19 quarentenas com zero RPC. Agora o modo guiado recebe também `<prior-coverage.json>` e deve registrar delta no próprio JSON.
- `guided-scenarios-v3/report.json` confirma pelo próprio Rust: cobertura anterior com 58 resultados, `new_root_outcomes=[]` e `new_root_jump_sites=[]`. O serviço transitório terminou com status 0. Próxima etapa técnica: planejar a partir de PCs não observados/um só sentido, com limites de estado e viabilidade; não alegar cobertura por função ou scanner completo.

## Etapa: aliases, efeitos, ordem e origem de função

- `variable_lens.rs` classifica pares de acesso indexado sem inventar igualdade dinâmica; registra relações possíveis entre escritores/leitores de estado, limites de chamadas a contrato e de assembly. O source map verificado atribui apenas PCs que cabem em uma única função/modifier da AST; não chamar isso de cobertura de caminhos alcançáveis. Veja `docs/VARIABLE-LENS.md`.
- `local_scenarios.rs` registra pré-checagem `eth_call`, resultados `SLOAD`/`SSTORE` do vault, continuidade entre etapas, slots escritos→lidos e candidatos de fórmula de mapping por endereço (bases 0..63). Candidato de slot não identifica sozinho variável de source.
- VPS: lente SSA v7 mostrou 41/110 PCs estáticos com origem única, 69 lacunas, 12 pares de chave, 11 relações possíveis de estado, seis limites de chamada e 36 blocos assembly. Cenários v6: 36 casos, 32 concluídos, quatro reversões, zero falhas/diferenças, quatro sequências contínuas com dois slots compartilhados cada; dois atores corresponderam a slots de mapping com base 0; zero novos resultados. Reavaliação manteve 29 zonas/19 quarentenas, zero RPC. Relatórios privados não vão ao Git.
- Permanecem limites matemáticos/externos: valores de chave não observados, efeitos internos de contratos externos, slots dinâmicos de assembly e alcançabilidade de todos os caminhos. Não marcar scanner geral como completo. Atualizar `codex/agent.md` e checklist após novas etapas; preservar `web/` não rastreado.

## Triagem local pré-IA (2026-10-19)

- `--evidence-filter` cruza hashes e identidade de M4, lente de variáveis, cenários no fork, cobertura derivada e pacote M5. Ordena variáveis já pontuadas e PCs de origem de source observados em menos de dois sentidos. Indica famílias para a próxima rodada. Veja `docs/EVIDENCE-FILTER.md`.
- M6 exige agora `--evidence-filter` no plano e na revisão. Somente uma diferença concreta vista em duas sequências distintas, sem falha RPC ou diferença pré-checagem/transação, pode liberar uma chave M5 para a IA. Sem chave elegível, zero chamadas. Isso é gate de priorização, não veredito.
- O modo guiado acrescenta até 16 valores próximos às escalas asset/shares e interrompe após oito casos sem resultado novo. Ainda não altera ordens/estados a partir de um PC específico em múltiplas rodadas. Não alegar busca exaustiva nem importância comprovada pelo score. Artefatos privados ficam só no VPS; `web/` não rastreado permanece intocado.

- VPS, rodada 2026-10-19: `toupera-evidence-feedback-20261019.service` terminou com status 0; 44 casos, 40 concluídos, quatro reversões esperadas, oito variações adicionais, parada por platô, zero decisões novas/erros RPC/diferenças. O filtro/planos privados mantiveram zero chaves/pedidos, sem IA. A ordenação final percorre até quatro relações de dependência a partir de usos econômicos. O próximo passo técnico é variar ordens e estados conforme lacunas específicas.

## Precisão v2 — seleção e leitura local (2026-10-19)

- A lente extrai `numeric_boundaries` de comparações diretas entre parâmetro e literal na AST de análise. O modo guiado varia abaixo/no/acima se couber no orçamento e, como fallback, localiza transições de `previewMint`/`previewRedeem` por consultas limitadas ao Anvil local. Os pontos são entradas candidatas; não há prova geral de monotonicidade ou equivalência ao runtime.
- O modo guiado acrescenta até oito sequências de ordem/estado isoladas por snapshot, com parada após quatro sem novo resultado no PC alvo. A rodada de valores mantém limite de 16 casos e parada por oito sem resultado no alvo. Somente Anvil em loopback recebe as transações locais.
- O filtro informa observações por função com denominador de PCs estáticos de origem única, candidatos de slot de mapping cruzados com layout, e alvos de chamada observados com mudanças de saldo concomitantes. Não atribuir causalidade de chamada, alias geral ou alcance a PC não observado. Artefatos privados ficam no VPS; `web/` fora dos commits.


Rodada `precision-v2` no VPS (2026-10-19): serviço `toupera-precision-transition-20261019.service` terminou com status 0. A lente listou 12 comparações diretas em funções auxiliares, nenhuma aplicável diretamente às quatro operações principais. A escolha por mudança de preview fez 68 consultas locais e originou seis casos; a rodada de valores parou após oito casos sem resultado novo no ponto alvo. Quatro sequências de ordem/estado foram executadas antes do próprio platô. Total: 50 casos, 46 concluídos, quatro reversões esperadas, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado permaneceu em 50/110 PCs estáticos, sem resultado novo; 41 PCs têm origem única de source, 23 deles observados. O filtro registrou 25 resumos por função, duas coincidências de slot/layout, quatro acessos indexados sem coincidência, um alvo de chamada observado e zero pedidos de IA. Todas as contagens se limitam ao estado e às entradas da rodada.

## Chip adaptativo de escolha local (2026-10-19)

- `adaptive_chip.rs` integra `--guided-scenarios`: memória limitada de operação/valor/classe, identidade e hash da rodada anterior, seleção por transição e distância, parada por classe/resultado novo. O argumento opcional final é o relatório de cenários anterior; a cobertura anterior precisa ter sido gerada por esse relatório. Ver `docs/ADAPTIVE-CHIP.md`.
- A classe resume status, pré-checagem, direção de mudanças de estado e resultados `JUMPI` raiz; não equivale a prova de caminho nem achado. Candidatos são apenas entradas do fork local. Limite de 16 casos de feedback, oito sem novidade e 256 observações guardadas. Preservar artefatos privados no VPS, `web/` fora dos commits e os gates de IA atuais.

Rodada precision-chip-v1 no VPS: serviço toupera-adaptive-chip-20261019.service com status 0; memória anterior de oito observações, 20 entradas repetidas e quatro vizinhos de mesma classe evitados, 61 candidatos ordenados. Oito casos de feedback sem nova classe/resultado alvo causaram parada. Total 50 casos (46 concluídos, quatro reversões), zero falhas RPC/diferenças, 50/110 PCs estáticos e 58 resultados observados, sem aumento. Filtro sem candidato e plano M6 vazio. Todos os artefatos brutos permanecem em /srv/toupera-agent/private-output/real-targets/yield-vault/precision-chip-v1/.

## Plano local e medição (2026-10-19)

- `forensic_plan.rs` oferece `--forensic-plan <manifest> <forensic> <lens> <scenarios> <coverage>` com hashes/identidade, condições simples da AST verificada, PC de faixa estreita quando único, classes de origem do fluxo abstrato do bytecode, valores verdadeiro/falso, pares de estado por slots observados, deltas e previews. `--guided-scenarios` aceita o plano como argumento final, executa os pares no Anvil filho e recusa plano encerrado antes de RPC. `--source-benchmark` mede fontes próprias; `tools/measure_local.py` mede valores em Anvil local. Ver `docs/SIMPLE-PLAN.md`.
- VPS: plano v1 a partir de `precision-chip-v1`: 12 condições então consideradas, zero ligação estreita, oito pares de estado. Serviço `toupera-simple-plan-20261019.service` terminou com status 0: 52 casos, 48 concluídos, quatro reversões, zero falhas/diferenças, quatro pares executados, nenhum resultado novo. Plano v2 ampliou a contagem de condições simples a 17, manteve zero ligações estreitas e registrou classes de origem para 46/102 PCs ainda com zero/um resultado. 240 deltas, 16 pares de preview e 48 diferenças contábeis sem divergência nesta rodada. Próxima rodada recusada por platô antes do RPC.
- Medição local de três cenários próprios: Rounding 15→16 resultados com 64→32 RPC; Modifier 9→12 com 48→12; Loop 13→11 com 48→12. A perda no loop é limite real da seleção curta. Artefatos brutos apenas em `private-output/`, nunca no Git. Manter `web/` fora do commit; atualizar `codex/agent.md` e checklist a cada etapa.

## Etapa: painel de um contrato wCOP
- O site passou a ter formulário e terminal de progresso para o endereço wCOP na Ethereum. A API local no VPS cria `smart.md`, passa runtime do endereço e implementação EIP-1967 ao Rust e cria `result_smart.md` e `end_smart_result.md`. Os artefatos brutos e históricos são privados.
- O Caddy protege a API por senha. A interface publica apenas o Markdown final a quem está autenticado. Não há envio de transações, relatório ao HackerOne ou chamada à IA.
- O relatório distingue dez ferramentas por estado real. Slither e Mythril são executados quando há entrada compatível; as demais exigem propriedades, compatibilidade ou chave específica e são marcadas sem simular sucesso. Ver `docs/SITE-WCOP.md`.
- Execução wCOP no VPS: bloco 26227621, proxy ligado à implementação `0x6f6206fac28d9f60d221a63aae5fb5c3a9ea6652`. Rust: 33 selectors e 475 incertezas na implementação. Slither executou sobre fonte verificada, 114 sinais brutos no conjunto, apenas um informativo com referência ao arquivo principal; Mythril produziu um sinal genérico de timestamp. Nenhuma falha confirmada.
- Configuração de deploy removida da raiz pública; modelo no Git contém apenas placeholder de hash. Segredo e hash ativos ficam somente no VPS. Recuperação de trabalho interrompido em reinício do serviço foi adicionada.
- A execução final via API autenticada, id `b0a86a00d5664e97`, concluiu oito eventos e publicou o Markdown final. O trabalhador lê e valida `smart.md` antes de obter o runtime. O servidor detecta processo encerrado ou ausência prolongada de progresso para não prender o painel em estado ativo.
- Foundry foi ligado ao painel wCOP com três regras locais em bloco fixado, via `tools/wcop_readonly.t.sol`. A execução manual no VPS passou em 3/3; isso cobre somente aprovação e duas operações sem papel. O trabalhador agora reproduz esse recorte automaticamente e registra a saída privada.
- Integração final `ba5146dd978d45e2`: nove eventos, Slither/Mythril/Foundry executados, Foundry 3/3 no bloco 26227659. Sete ferramentas restantes têm estado motivado, sem alegação de resultado. `end_smart_result.md` disponível no painel autenticado.


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


## Etapa de mapa e medição do wCOP (2026-10-19)

- `tools/wcop_scope.py` recompila as 34 fontes verificadas com Solidity 0.8.27 e configurações publicadas; compara runtime ignorando apenas posições imutáveis declaradas pelo compilador. A comparação inicial conferiu sem esses três campos.
- O mapa da ABI confronta cada seletor com o Rust e liga funções somente a cenários que passaram. Na rodada inicial, 33/33 seletores coincidiram; 18 cenários locais passaram, incluindo papéis ativos, autorização, assinatura, emissão/retirada e sequências variáveis.
- `tools/wcop_coverage.t.sol` mede cobertura da fonte recompilada em um proxy local. A primeira leitura mostrou 22/22 linhas próprias e 286/1126 linhas, 19/213 ramos no conjunto fonte e bibliotecas. Esta medida não é cobertura direta do bytecode no bloco.
- `tools/wcop_properties.sol` fornece duas regras de conservação para sequências locais; Echidna e Medusa passaram em campanhas curtas no VPS. `tools/wcop_formal.t.sol` existe para prova limitada; Halmos não concluiu porque o preparo do proxy usa uma operação não suportada no executor atual. Relatar como inconclusivo.
- O fluxo do site precisa publicar os números reais e as lacunas, sem anunciar certeza de 100%. Gravar detalhes somente em `private-output`. Depois da rodada final, criar novo documento Markdown no Git com os resultados e limites.


### Resultado integrado do mapa (2026-10-19)

- Trabalho `ac1f94183eeb4c5e`: 33/33 seletores da ABI reconhecidos pelo Rust, 33/33 entradas com ao menos um cenário local passado, Foundry 18/18 no bloco, fonte local 6/6. A recompilação com 34 fontes conferiu após mascarar três posições imutáveis declaradas pelo compilador.
- Echidna e Medusa concluíram duas regras locais de conservação em sequências curtas. Halmos permaneceu inconclusivo por limite no preparo do proxy; não registrar prova formal. Certora e Kontrol continuam sem prova neste fluxo.
- Cobertura local: `LatamStable.sol` 22/22 linhas, 7/7 funções, 0/0 ramos mensuráveis; conjunto de fonte e bibliotecas 286/1126 linhas, 19/213 ramos. Muitas rotinas genéricas importadas podem ser inalcançáveis pelo token; a soma não representa porcentagem do runtime publicado.
- `docs/WCOP-COBERTURA-2026-10-19.md` foi gerado a partir do JSON privado, com mapa por função e lacunas. Antes de publicar no Git, conferir que não contém credenciais nem dados da pasta privada.


## Segunda etapa de caminhos wCOP (2026-10-19)

- A compilação local desativou `dynamic_test_linking` no Foundry para que Halmos processe o proxy. Três propriedades limitadas de oferta, transferência e pausa concluíram; não são prova de todos os estados.
- Foram acrescentados casos locais de valores inválidos, permissões, assinatura vencida, allowance máximo e troca válida de implementação. A medição intermediária chegou a 12/12 casos e 37/213 ramos no conjunto compilado, contra 19/213 na rodada anterior.
- `tools/wcop_scope.py` agora informa separadamente arquivos com linhas executadas. Esse recorte depende dos casos executados e não representa todos os caminhos possíveis.
- A etapa seguinte mede também preservação de pausa, allowance, nonce, domínio, papéis e saldos após troca local. Confirmar números no novo trabalho antes da publicação.


### Continuidade do histórico (2026-10-19)

- Uma rodada recebeu limite 429 na API antiga de eventos do explorador; o resultado ficou com histórico incompleto e um conjunto reduzido de casos no bloco. Esse resultado não substitui a rodada completa.
- `tools/wcop_history.py` agora consulta primeiro as páginas v2 do mesmo explorador, normaliza os registros e mantém a API antiga como alternativa. Em leitura isolada no VPS, v2 trouxe 236 eventos em 5 páginas e a oferta conferiu com a rede.
- `tools/site_jobs.py` faz tentativas limitadas em respostas temporárias e calcula a quantidade esperada de casos a partir do arquivo Foundry, corrigindo a contagem do conjunto reduzido.


### Resultado final da segunda etapa (2026-10-19)

- Trabalho completo `2d97ca57278541bc`, bloco `26227913`: 236 eventos, reconciliação da oferta, 33/33 entradas ABI mapeadas, 18/18 cenários no bloco, 14/14 na fonte local, duas regras Echidna, duas Medusa e três propriedades Halmos limitadas.
- A recompilação de 34 fontes continuou compatível com o runtime após mascarar somente as três posições imutáveis declaradas. A troca local preservou saldo, autorização, nonce, domínio de assinatura, papel e pausa nos casos executados.
- Cobertura local passou de 19/213 para 38/213 ramos no conjunto compilado. Nos 18 arquivos com linhas executadas houve 38/77 ramos; o arquivo próprio ficou em 22/22 linhas. As 136 ramificações nos arquivos sem linha executada não devem ser interpretadas automaticamente como alcançáveis ou inalcançáveis.
- `docs/WCOP-COBERTURA-ETAPA2-2026-10-19.md` traz a nova medição e as lacunas ativas. Continua sem medição de ramos do bytecode publicado, sem prova de todos os estados e sem confirmação de falha.


## Etapa de caminhos do runtime wCOP (2026-10-19)

- `tools/wcop_trace.py` cria uma cópia Anvil do bloco fixado, confere o slot EIP-1967 e o runtime da implementação contra `implementation.hex`, executa chamadas isoladas e uma sequência local com estado, e registra PCs, blocos e transições da implementação. A cópia escuta somente em `127.0.0.1`; não envia transações à rede pública.
- `security-engine --wcop-trace` confere o hash do código normalizado, valida que os PCs e blocos observados pertencem ao mapa, separa ligações observadas fora do CFG estático e ordena dúvidas situadas em PCs visitados. Essa ordem não indica falha confirmada.
- Ensaio isolado sobre o trabalho `2d97ca57278541bc`: 36 chamadas, das quais 12 em sequência; 10 passos gravados apenas na cópia; 0 erros e 0 divergências de resultado esperado. Foram observados 2470/5377 PCs e 241/559 blocos; 61 transições observadas ainda não constam do CFG estático. Esses denominadores não são prova de alcance total.
- O pipeline do painel foi ampliado para registrar a leitura automaticamente quando houver histórico, papéis e implementação. Conferir a rodada integrada antes de publicar o documento `docs/WCOP-CAMINHOS-RUNTIME-2026-10-19.md` no Git privado.


### Resultado integrado dos caminhos do runtime (2026-10-19)

- Trabalho `154aa11a5aa144ed`, bloco `26227976`: histórico 236 eventos e oferta reconciliada; 18/18 cenários Foundry no bloco, 14/14 na fonte local, 3/3 propriedades Halmos limitadas.
- A etapa automática `Caminhos` concluiu 36 chamadas locais, incluindo 12 numa sequência com 10 passos gravados apenas na cópia. Não houve erro de leitura nem divergência de resultado esperado.
- O Rust conferiu a identidade do código e registrou 2470/5377 PCs e 241/559 blocos observados; 235 transições coincidem com arestas estáticas e 61 transições observadas ainda não constam do mapa. Havia 202 dúvidas do motor em PCs visitados; nenhuma é falha confirmada por isso.
- `docs/WCOP-CAMINHOS-RUNTIME-2026-10-19.md` resume casos, números, limites e próximos passos. Logs, PCs completos e ligação detalhada ficam no VPS privado. A medição não é cobertura total de estados ou de instruções alcançáveis.

## Arquitetura de confiança proposta (2026-10-19)

- A pedido do usuário, a arquitetura foi definida antes de qualquer nova implementação. Ver docs/ARQUITETURA-CONFIANCA-2026-10-19.md.
- Fluxo previsto: identidade fixa → validação de fonte/AST e bytecode → núcleo Rust → ferramentas locais → guardião de evidências Rust → painel/revisão. O guardião classifica afirmações e preserva premissas; não controla transações na rede.
- Estado desta etapa: somente documentação. Estão pendentes manifesto de identidade, verificador AST, decisão estruturada e ligação ao painel. Continuar a atualizar este arquivo, codex/agent.md e checklist a cada etapa. Código e artefatos privados permanecem no VPS.

## Etapa 1: entradas e identidade (2026-10-19)

- O compilador agora solicita AST de todas as fontes e rejeita nomes repetidos, caminhos inválidos e entradas acima dos limites. O trabalhador cria job-manifest.json com hashes dos artefatos e chama o futuro guardião Rust. Esta etapa ainda depende do novo CLI; não reiniciar o serviço antes da integração.

## Etapa 2: guardião de evidências integrado (2026-10-19)

- CLI Rust --wcop-guard valida manifesto, arquivos/AST, cópias de fonte, binários, slot EIP-1967, proxy, implementação e comparação recompilada. Saída por afirmação em decision.json; fonte divergente cai para bytecode_parcial.
- tools/site_jobs.py reconfirma o bloco, cria manifesto e põe a decisão no início de end_smart_result.md; em falha, suspende relações de fonte. O modo bytecode-only foi conferido em cópia privada.
- Trabalho integrado 3932c3f9ca4e4faf concluiu no VPS: 34 ASTs, identidade válida, vínculo masked_immutables com ressalva, relatório visível no painel. Faixa AST adulterada foi rebaixada em cópia privada; não houve transação pública nem conclusão de falha.
- Ver docs/GUARDIAO-EVIDENCIAS-2026-10-19.md. O isolamento por permissões e o vínculo opcional de decision.json com M8 foram concluídos nas etapas 3 e 4; não atribuir ao projeto provas matemáticas de seL4/CompCert.


## Etapa 3: isolamento do serviço (2026-10-19)

- O serviço do painel usa ProtectSystem=strict, PrivateTmp, sem capacidades adicionais e com gravação limitada a private-output e caches necessários. Código, scripts, binário e diretórios do projeto pertencem ao root e ficam somente para leitura do usuário toupera.
- Os três arquivos Markdown da raiz continuam acessíveis por links para private-output/root-files; as escritas atômicas preservam esses links. Um processo de ferramenta não consegue alterar site_jobs.py ou security-engine pelo usuário do serviço.
- A política foi aplicada e o serviço reiniciado. systemd-analyze security passou de 9.0 UNSAFE para 4.8 OK. Nesta etapa ainda não havia isolamento individual; a etapa 5 acrescenta namespaces por execução. Não declarar isolamento formal completo.


## Etapa 4: vínculo do guardião com M8 (2026-10-19)

- `security-engine --evidence-plan <case-dir> <output-dir> --guard-job <job-dir>` exige decisão atual idêntica ao decision.json salvo, identidade válida, fonte vinculada e hash da fonte principal igual ao M7. O pacote copia decisão, manifesto do trabalho e entrada do compilador com SHA-256.
- `--evidence-finalize` confere de novo os hashes e a ligação entre decisão, endereço, bloco, implementação, manifesto e fonte antes da revisão humana. Casos M8 sem `--guard-job` continuam possíveis, com limitação explícita no manifesto; não recebem vínculo de contrato publicado.
- Campos imutáveis mascarados são anotados como ressalva. O guardião não transforma um ensaio ou hipótese em resultado confirmado. A finalização permanece dependente dos dois resultados Foundry e da revisão humana já exigidos por M8.

- A primeira rodada após o isolamento teve 17/18 cenários Foundry por HTTP 502 do RPC. O trabalhador agora repete somente respostas transitórias e preserva os logs no VPS.

- Rodada posterior `d59cb58cee084b10`: 18/18 cenários Foundry locais, decisão vinculada à fonte com ressalva dos campos imutáveis e serviço restrito ativo. Ver `docs/ISOLAMENTO-E-VINCULO-M8-2026-10-19.md`.


## Etapa 5: executor por trabalho (2026-10-19)

- `command()` executa ferramentas por `bubblewrap`: raiz somente para leitura, área privada do trabalho atual gravável, `/tmp` temporário e namespaces separados de montagem, PID, IPC e host. Os demais trabalhos são ocultos na visão do subprocesso.
- Mythril recebe cópia privada de seu cache no trabalho; o diretório do provedor não é gravável pela ferramenta. `wcop_trace.py` e Anvil local também passam pelo executor. O serviço exige `/usr/bin/bwrap` no início.
- A rede continua compartilhada para leituras RPC e consulta de fonte. Não há isolamento de rede por ferramenta nem garantia formal; o processo coordenador do painel mantém acesso aos trabalhos para reunir resultados.

- Rodada final `3fdf10704a64482c`: isolamento por execução ativo, outro trabalho oculto, seis ferramentas aplicáveis concluídas, 18/18 cenários locais, caminhos reconciliados e decisão Rust vinculada com ressalva. Pacote M8 ligado permanece `PENDING_REPRODUCTION`. Ver `docs/ISOLAMENTO-E-VINCULO-M8-2026-10-19.md`.


## Ciclo adaptativo local — etapa 1 (2026-10-19)

- O Rust ganhou `--wcop-adapt-plan <engine.json> <state.json>`. Ele valida o runtime fixado, aceita apenas candidatos limitados do wCOP, lê as observações anteriores e escolhe a próxima combinação por ganho real de PCs/transições, famílias pouco usadas e blocos ainda não vistos. O limite é 32 tentativas ou doze sem ganho.
- `tools/wcop_adaptive.py` executa a escolha só na cópia Anvil do bloco. Gera combinações de transferência, autorização, emissão, retirada, pausa e papéis a partir de contas observadas; após cada resultado entrega ao Rust o ganho visto. Compara variações de oferta, saldos, autorização e estado de pausa; divergências ficam como sinais para revisão, não como falhas confirmadas.
- O trabalhador chama o ciclo após a leitura inicial, confere os caminhos pelo Rust e inclui os arquivos `adaptive*.json` no manifesto. O guardião verifica identidade, hashes, bloco e contagens antes de classificar a afirmação. Não há envio de transações à rede pública.
- Trabalho integrado `fe5aa294573f4571`, bloco `26228348`: 18 escolhas, 7 operações aplicadas somente na cópia local, 34 PCs e 2 transições novos além da leitura inicial (2504/5377 PCs em conjunto). O ciclo parou após doze escolhas sem ganho; não houve diferença registrada nas verificações de estado feitas. A reconciliação Rust teve zero erros e o guardião marcou apenas a afirmação limitada do ciclo como `OBSERVADO`.
- O executor aguarda o recibo local antes de considerar uma operação aplicada e preserva valores grandes como inteiros decimais exatos entre Python e Rust. Detalhes e limites: `docs/CICLO-ADAPTATIVO-2026-10-19.md`.

## Etapa 2 do ciclo — andar pelo mapa (2026-10-19)

- O Rust passou a olhar para saídas adjacentes ainda não visitadas e para o ganho real das escolhas anteriores. Ele escolhe entre operações simples e sequências curtas, com limite de 40 escolhas.
- O executor prepara estado apenas na cópia local, mede o caminho seguinte e desfaz a preparação. Valores vizinhos de uma escolha produtiva também entram nas opções; autorização sem limite e assinaturas com valores distintos ampliam o mapa de decisões.
- Trabalho integrado `f7c70e446f134e78`, bloco `26228428`: 40 escolhas, 18 sequências, +578 PCs, +50 ligações, +38 blocos e 11 passagens para blocos novos; conjunto observado 3048/5377 PCs. Zero diferenças nas verificações feitas; isso não é prova de todos os estados. A medição de ramificações da fonte continua 38/213, pois este ciclo mede bytecode.
- Resultado e limites em `docs/CICLO-ADAPTATIVO-2026-10-19.md`. `agente.md`, `codex/agent.md` e checklist devem continuar atualizados após cada nova etapa. Código e arquivos privados ficam no VPS; Git privado recebe apenas fonte e documentação.

## Nova rodada completa e fila de revisão (2026-10-19)

- Trabalho `7011fcd042db41e5`, wCOP Ethereum bloco `26228465`: o VPS gerou `smart.md`, `result_smart.md` e `end_smart_result.md` de novo. O relatório `docs/WCOP-RODADA-7011fcd042db41e5.md` guarda hashes dos três arquivos e analisa os resultados do mesmo bloco.
- O motor Rust e o ciclo local ligaram 3048/5377 PCs; 40 escolhas acrescentaram 578 PCs, 50 ligações e 38 blocos. A fonte recompilada mediu 38/213 ramos; é outra medida e não foi atualizada pelo ciclo do bytecode.
- Todos os 236 registros da rede, 40 escolhas locais, 30 dúvidas selecionadas do Rust e 114 sinais brutos do Slither receberam pontos de atenção no relatório. Pontos altos são ordem de revisão, não falha confirmada. Não houve diferença registrada nas verificações de estado; Foundry 18/18, Echidna 2/2, Medusa 2/2, Halmos 3/3 sob seus limites.
- A ligação com o VPS foi feita por SSH com verificação estrita da chave e cifra `chacha20-poly1305@openssh.com`. Arquivos de trabalho continuam privados no VPS; código e relatório foram destinados ao Git privado.

## Mapa de linhas e permissões — 2026-10-19

- O compilador fornece source map do runtime; `tools/wcop_path_map.py` confere vínculo e liga PCs das duas leituras a arquivo/linha. O painel inclui o mapa no manifesto; o verificador Rust valida identidade e hash.
- Em aplicação retrospectiva ao trabalho `7011fcd042db41e5`, 1945/3048 PCs observados receberam local de fonte; 74 ligações observadas ficaram fora do mapa estático combinado. O mapa termina no PC 8499 e 37 posições desmontadas depois dele não foram observadas.
- Registros de papéis (15, cinco contas) foram refeitos offline e coincidiram com a consulta de estado guardada para quatro papéis. A descoberta de todas as contas permanece não comprovada.
- Ver `docs/WCOP-MAPA-LINHAS-2026-10-19.md`. Nenhum novo trabalho completo foi executado nesta etapa. Continuar a atualizar este arquivo, `codex/agent.md` e o checklist após cada etapa.

## Etapa: arquivos internos do compilador (2026-10-19)

- `wcop_path_map.py` agora pede e lê os arquivos internos gerados pelo compilador, preservando a distinção entre fonte original, conteúdo gerado e posição sem local.
- Na leitura retrospectiva `7011fcd042db41e5`, posições com local subiram de 3168 para 4463 no mapa total e de 1945 para 2504 entre 3048 posições observadas. 559 posições observadas foram ligadas a conteúdo gerado.
- O vínculo de runtime e campos imutáveis continua obrigatório. Esta etapa ainda não muda seleção de cenários nem afirma cobertura completa.

## Etapa: seleção Rust por linhas e ligações (2026-10-19)

- Novo `--wcop-focus` em Rust confere identidade e calcula prioridades por entrada da ABI a partir de linhas não observadas e ligações vistas fora do mapa estático. A saída preserva os 74 itens para revisão do mapa.
- O trabalhador prepara `focus.json` antes do ciclo local e `focus-final.json` depois; o Rust usa a prioridade inicial com retorno decrescente por repetição.
- O executor local recebeu duas combinações de duas preparações. Orçamento e parada do ciclo permanecem limitados; descoberta de diferença não é garantida.
- A compilação de produção terminou no VPS. Ainda falta integrar histórico de versões e filtro final desta rodada.

## Etapa: linha de versões (2026-10-19)

- `wcop_versions.py` consulta somente os blocos fixos do histórico e guarda código, posição da implementação, oferta e permissões das contas vistas.
- O Rust `--wcop-versions` reconcilia registros de mudança, blocos e hash da versão final; diferenças ficam explícitas.
- `versions-raw.json` e `versions.json` entram no trabalho privado quando a leitura conclui. Blocos de evento não mostram a ordem de todas as operações internas nem descobrem todas as contas.

## Etapa: filtro final (2026-10-19)

- Rust `--wcop-review-gate` lê decisão, ciclo, mapa final e versões no mesmo trabalho. Só libera um pacote compacto quando há diferença repetida na cópia local e todas as ligações necessárias estão observadas.
- Caminhos novos sem diferença ficam em revisão local. O filtro não chama IA automaticamente, não marca resultado confirmado e não converte contagem de caminhos em conclusão.
- O painel mostra o estado do filtro ao fim do trabalho; ausência de entrada permanece explícita.

## Ajuste da linha de versões (2026-10-19)

- O provedor respondeu 403 para estado de bloco antigo. O coletor agora grava essa lacuna, usa o estado atual do trabalho e deixa o Rust classificar a linha como limitada.
- A aplicação retrospectiva `7011fcd042db41e5` encontrou uma troca registrada, dois blocos de interesse, uma leitura histórica indisponível e concordância entre a última implementação registrada e a atual.
- O trabalho integrado foi reiniciado após corrigir a preparação do diretório de logs. Nenhum trabalho incompleto é apresentado como resultado final.

## Rodada integrada após as novas etapas (2026-10-19)

- Trabalho `7d78331aa23a4592` concluído no VPS, bloco `26228635`, 236 registros de rede reconciliados. O Rust ordenou 33 entradas e 74 ligações, o ciclo fez 38 escolhas com +544 PCs e parou por platô.
- Mapa final: 2478/3014 PCs observados receberam local do compilador, incluindo 541 de arquivos internos. O resultado não mede todos os caminhos alcançáveis.
- Linha de versões `LIMITADO` por estado antigo indisponível; código atual e último registro concordam. Guardião conferiu identidade e mapa; filtro final ficou em `local_review`, zero itens para IA e zero diferenças locais registradas.
- Ver `docs/WCOP-RODADA-7d78331aa23a4592.md`.


## Etapa: limite do mapa na escolha Rust (2026-10-19)

- O Rust agora valida o fim do mapa do compilador e separa as posições posteriores do orçamento que orienta as escolhas. O executor mantém ganhos totais e ganhos dentro do intervalo do mapa em campos diferentes; posições finais continuam visíveis no relatório.
- Conferência offline no VPS com o trabalho `7d78331aa23a4592`: PC final 8499, 37 posições posteriores e zero observadas. Ao marcar todos os PCs dentro do intervalo do mapa como vistos em um estado temporário, o orçamento de PCs dentro do intervalo do mapa ficou zero; as 37 posições permaneceram separadas.
- `py_compile`, `cargo check --offline` e `cargo build --release --offline` passaram no VPS. A primeira compilação release como toupera encontrou um artefato de root sem permissão; a repetição como root concluiu.
- Arquivos alterados: `wcop_focus.rs`, `adaptive_cycle.rs`, `wcop_adaptive.py`, este arquivo, `codex/agent.md`, checklist e `docs/WCOP-FOCO-MAPA-2026-10-19.md`.
- Nenhuma nova rodada do contrato foi feita; a mudança não demonstra maior cobertura nem uma diferença confirmada. Próximo passo: separar posições internas sem linha e relacionar caminhos do runtime a ramos da fonte. Ver o documento da etapa.


## Rodada após o ajuste do mapa (2026-10-19)

- Trabalho `92273047cbf7439c` terminou no VPS no bloco `26228846`. O Rust validou o limite PC 8499; as 38 escolhas acrescentaram 544 PCs dentro do intervalo do mapa e zero nas 37 posições posteriores. Total observado 3014/5377 PCs; 2478 observados com local.
- Fonte recompilada 38/213 ramos, 74 ligações observadas fora do mapa estático, filtro `local_review` com zero itens externos e zero diferenças locais repetidas. Estado histórico antigo permanece indisponível.
- As contagens coincidem com a rodada anterior em bloco distinto; não há ganho de cobertura demonstrado. Os três Markdown e logs do trabalho permanecem privados no VPS. Código e documentação foram publicados no Git privado.
- Ver `docs/WCOP-RODADA-92273047cbf7439c.md`. Próximo passo: posições sem linha dentro do mapa e vínculo entre runtime e ramos da fonte.
