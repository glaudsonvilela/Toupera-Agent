# Checklist — da fundação ao relatório

Esta lista acompanha a construção do Toupera Agent e o ciclo completo de análise de cada alvo. Uma hipótese só avança com evidência suficiente.

## Homepage pública — Toupeira Agente

- [x] Criar homepage responsiva em `homepage/`, com a marca alinhada a `toupeiraagente.tech` e a referência visual do usuário.
- [x] Implementar fumaça, partículas, luz, parallax e etapas interativas, com pausa e respeito a movimento reduzido.
- [x] Conferir desktop/mobile, navegação por teclado, controles e arquivos da página.
- [x] Publicar a raiz do domínio no VPS com HTTPS e verificar os redirecionamentos (200 IPv4/IPv6, HTTP 308, www 301).

Esta homepage apresenta a tecnologia; não executa análises públicas nem expõe os relatórios privados do motor.

## Construção do motor

### M0 — Fundação
- [x] Repositório privado, estrutura, documentação e imagem oficial.
- [x] Crate Rust com tipos fundamentais e testes mínimos.

### M1 — IR econômica com proveniência
- [x] Definir IDs de contrato, função e localizações de evidência.
- [x] Representar operações deposit/mint/withdraw/redeem, previews e conversões.
- [x] Representar storage lido/escrito, chamadas, oracles e expressões aritméticas.
- [x] Associar cada elemento ao source span e preparar PC de bytecode para etapa futura.
- [x] Marcar confiança, semântica incerta e dados ausentes explicitamente.
- [x] Validar a IR com fixtures pequenas de vaults Solidity.

### M2 — Reconstrução source-first (concluído no recorte de um arquivo)

- [x] Ingerir um arquivo Solidity explícito e obter AST do solc local.
- [x] Construir fluxos de controle, chamadas e dependências de estado.
- [x] Mapear operações econômicas, caminhos de oracle e conversões shares/assets.
- [x] Identificar proxies/implementações quando houver evidência.
- [x] Comparar a IR com fixtures e resultados locais conhecidos.

O recorte M2 cobre um arquivo Solidity sem imports. O grafo estrutural source-level liga `if`, retornos, `require`/`assert`, ternários simples, laços e modifiers com um `_`, inclusive herdados no mesmo arquivo. Chamadas internas preservam o contrato declarante; caminhos e fluxo de valor conservador chegam a storage, oracle e conversões com proveniência. `delegatecall` com implementação `immutable` criada localmente no construtor é ligado ao contrato alvo; endereços externos continuam candidatos incertos. Laços e chamadas desconhecidas mantêm incertezas. `try`, assembly, múltiplos ternários, imports e bytecode permanecem fora deste recorte; M3 cobre bytecode.

### M3 — Engenharia reversa de bytecode EVM (concluído no recorte legado conservador)
- [x] Normalizar runtime hex legado e remover metadados CBOR Solidity apenas quando reconhecidos.
- [x] Recuperar dispatch direto de selectors, blocos básicos e CFG de saltos literais ou constantes propagadas.
- [x] Observar chamadas e storage por PC; inferir dependências genéricas entre entradas, leituras, retornos de chamadas, aritmética e sinks econômicos.
- [x] Registrar observações e inferências recuperadas com PC, confiança e incertezas.
- [x] Manter comportamento ambíguo como desconhecido; comparar runtime normalizado e métodos com saída local do solc quando disponível.

O fechamento M3 cobre somente runtime EVM legado fornecido explicitamente: interpretação abstrata limitada de stack e memória por palavras, com limite de propagação. Os caminhos econômicos são dependências genéricas até escrita de storage, valor/alvo/entrada de chamada, condição ou retorno, sem atribuir identidade de oracle, asset ou share ao bytecode isolado. Dispatch fora do padrão direto, saltos ainda dinâmicos, memória parcial, ABI desconhecida, EOF e creation code permanecem em `unresolved` ou fora do recorte. A comparação local com source só atribui rótulos econômicos quando o runtime normalizado coincide byte a byte ou quando toda divergência está em faixas `immutableReferences` fornecidas pelo compilador; as duas condições ficam separadas no JSON. Não é decompilador completo nem detector de vulnerabilidade. Ver [reconstrução de bytecode](BYTECODE-RECONSTRUCTION.md).

Complemento após M3: EVMole 0.9.4 fornece uma segunda leitura local de selectors, argumentos, mutabilidade, storage e CFG. Suas propostas permanecem separadas e tentativas; divergências são registradas, não incorporadas automaticamente ao grafo nativo. A comparação com source pode aproveitar selector ABI exclusivo do EVMole, ainda com confiança `Tentative`.

Complemento adicional: Heimdall ABI e Gigahorse TAC podem ser importados com `--bytecode-companions` ou gerados pelo wrapper local. Selectors são correlacionados com PCs nativos/EVMole; blocos e sucessores TAC são correlacionados com PCs decodificados. Nenhuma dessas propostas entra automaticamente em fluxo econômico ou achado de vulnerabilidade.

### M4 — Análise econômica determinística (implementação source-backed; calibração pendente)
- [x] Codificar triagem estrutural de conservação assets/shares e assimetria de preço, sem alegar prova numérica.
- [x] Emitir candidatos para escalas decimais, arredondamento, preview/execução e accounting com evidências observadas.
- [x] Emitir candidatos para sensibilidade de primeira emissão/doação, fee e caminhos de oracle sob premissas explícitas.
- [x] Importar JSON local do Slither como evidência complementar e deduplicar por contrato, função e categoria na IR Rust.
- [x] Emitir apenas candidatos `UNVERIFIED` com spans, regras, premissas e confiança tentativa.
- [ ] Medir redução de candidatos, falsos positivos e cobertura em fixtures: adiado a pedido do usuário até a implementação completa.

O M4 usa a IR source-first de um arquivo sem imports. Regras sintáticas não provam conservação de quantidades, round trips ERC-4626, possibilidade de doação ou qualidade de oracle. O bytecode M3 permanece evidência estrutural separada. A saída e os limites estão em [Análise Econômica](ECONOMIC-ANALYSIS.md).

Lacuna observada no piloto M7: `--llm-plan` retornou zero candidatos para `RoundingVaultM7Fixture.sol`, apesar da divergência sintética de arredondamento reproduzida. Ampliar e calibrar a detecção M4 antes de considerar o pipeline completo validado.

A regra `rounding-preview-formula-difference` foi acrescentada para a diferença literal `x / d` versus `(x + d - 1) / d` com o mesmo numerador/denominador. A calibração no VPS e o ramo positivo completo ainda devem ser conferidos antes de marcar o gate como concluído.

Calibração no VPS: a fixture M7 gerou 1 candidata `UNVERIFIED` com 2 source spans; M5 produziu 1 slice e o plano offline M6 selecionou 1 pedido. A candidata cobre uma possibilidade condicionada por `faultyPreview`; não é veredito sobre o ramo corrigido. Não houve chamada de IA, novo caso M7 ou pacote M8 nessa passagem.

### M5 — Deduplicação e contexto mínimo
- [x] Calcular hashes de source, runtime opcional normalizado, AST e função, além de compiler e motor.
- [x] Evitar reanálise de entradas idênticas com cache local por conteúdo.
- [x] Montar slices limitados com função, chamadas, storage, aritmética, oracle e invariantes relevantes.
- [x] Manter saída local sem envio ao LLM ou do repositório inteiro por padrão.

O M5 opera no recorte source-backed do M2/M4, um arquivo sem imports. Runtime opcional é só fingerprint; não é ligado ao source sem comparação separada. O cache privado guarda o pacote reduzido e invalida ao mudar entrada, compilador, motor ou versão EVM informada. Os limites e o formato estão em [Pacote de Contexto](CONTEXT-PACKAGE.md). Economia de tokens e taxa de acertos permanecem sem benchmark geral.

### M6 — Hipóteses do LLM
- [x] Definir interface Rust desacoplada do fornecedor e primeiro adaptador DeepSeek.
- [x] Preparar somente contexto reduzido e estruturado, com envio externo explícito.
- [x] Exigir hipótese, premissas, caminhos relevantes e teste sugerido, ou resposta de evidência insuficiente.
- [x] Manter o achado como `UNVERIFIED`; LLM não confirma vulnerabilidade.
- [x] Registrar chamadas/tokens, limitar candidatos e reaproveitar respostas válidas idênticas em cache privado.

O M6 foi implementado no recorte de candidatos source-backed do M4/M5. Preparação offline e chamada externa são comandos distintos; a API não foi acionada nesta etapa. O custo estimado requer rate card local com tarifas atuais. Prompt, contrato de saída e limites em [Revisão com IA](LLM-REVIEW.md). Qualidade e economia de tokens seguem sem medição até a validação geral solicitada pelo usuário.

### M7 — Juiz Foundry
- [x] Preparar caso privado por hipótese priorizada, com snapshot do alvo e scaffold Foundry explicitamente pendente de implementação humana.
- [x] Implementar executor local com seed fixa, duas execuções, limites de tempo e gates de harness revisado.
- [x] Instrumentar contagem observada de fuzz, LCOV do alvo e contadores de chamadas/reverts de invariantes quando houver campanha.
- [x] Implementar harness específico e reproduzir divergência em fixture própria de fee e arredondamento.
- [x] Executar apenas em testes unitários locais autorizados; nenhum fork ou contrato real no piloto.
- [x] Aplicar fuzzing Foundry a domínio válido, fee e arredondamento; conferir limites determinísticos de 1 a 7.
- [x] Implementar rejeição revisada e exercitá-la no ramo corrigido, sem transformar falha de compilação/teste em rejeição automática.
- [x] Reexecutar resultado positivo com seed fixa e guardar JSON, traces e LCOV das duas rodadas.
- [x] Medir cobertura e documentar contagem inferida de chamadas econômicas válidas/reverts no piloto; decidir Echidna/Medusa somente por lacuna ou achado único.
- [x] Executar piloto Halmos para propriedade curta e documentar decisão de adiar Kontrol/Certora.

O M7 do executor local está concluído no recorte sintético descrito em [Piloto M7](M7-PILOT.md). Uma aprovação declarada em `harness.json` é condição de entrada, não prova de qualidade do teste. Contadores de invariant handlers não provam chamadas válidas ao alvo. O motor não converte texto da IA em Solidity, não rejeita hipótese automaticamente com uma execução falha e não confirma achado por um teste verde. A triagem M4 não selecionou a fórmula do piloto; casos reais exigirão seu próprio harness, escopo e revisão. Ver [Juiz Foundry](FOUNDRY-JUDGE.md).

Critérios e prioridade por ferramenta: [Estratégia de ferramentas](TOOLING-STRATEGY.md).

### M8 — Evidência e entrega
- [x] Implementar pacote privado com hash do source, configuração, harness, JSON e traces, além de campos obrigatórios de versão e reprodução na revisão humana.
- [x] Implementar separação explícita de fatos, inferências, premissas e limitações no manifesto e relatório.
- [ ] Fazer revisão humana de escopo, impacto e duplicidade.
- [x] Implementar geração de relatório com evidência mínima e gate de revisão; nenhum relatório real gerado nesta etapa.
- [x] Implementar gate que só marca `REPORT_READY` após duas execuções positivas, integridade dos artefatos e atestados de revisão humana; gate não acionado nesta etapa.

O M8 está implementado no recorte de um arquivo source-first, com reprodução e revisão reais pendentes. `--evidence-plan` pode criar um pacote `PENDING_REPRODUCTION`; `--evidence-finalize` falha até haver um resultado M7 positivo e revisão humana completa. Ver [Evidência e entrega](EVIDENCE-DELIVERY.md).

### Piloto em contrato EVM real — Sepolia, 2026-10-18

- [x] Fixar endereço, chain ID, bloco, hash do runtime e source verificado de `YieldVault`.
- [x] Recompilar o bundle multifile com as configurações do deploy e comparar runtime, incluindo faixas `immutable`.
- [x] Passar a visão achatada por M1/M2, runtime por M3, e unir a triagem Slither no pacote M4/M5.
- [x] Registrar plano M6 vazio e provar que nenhuma chamada DeepSeek foi necessária.
- [x] Confirmar que M7 gera zero casos e que M8 não libera relatório sem caso.
- [ ] Validar o ramo automático **com candidato** M4→M5→M6→M7→M8 em alvo autorizado ou fixture calibrada; o contrato Sepolia não produziu candidato.
- [x] Reconstruir mapa forense de zonas sensíveis e priorizar caminhos com justificativas de source, runtime e incertezas.
- [x] Encaminhar zonas prioritárias a uma segunda passagem de quarentena com leitores/escritores, fluxos, PCs e sondagens `eth_call` no bloco fixado.
- [x] Refazer o mapa após a quarentena e conectar anomalias observadas ao M5 somente com vínculo de identidade; no alvo real nenhuma anomalia foi observada.
- [ ] Validar em alvo autorizado o ramo de quarentena **com anomalia real**, hipótese M6, reprodução M7 e revisão M8.

Dados e limites do piloto: [YieldVault na Sepolia](REAL-EVM-PILOT.md). Zero candidatos é parada correta para este alvo, não uma avaliação de segurança completa nem prova de conexão do ramo positivo.

## Ciclo por alvo auditado

### Evolução do scanner forense automático (em andamento)

- [x] Fixar identidade do source, runtime, chain ID e hash do bloco; gerar mapa inicial e quarentena com prioridades explicáveis.
- [x] Usar inteiros EVM de 256 bits para decodificar e reconciliar valores sem truncamento.
- [x] Varredura de eventos desde o deploy quando o RPC fornece histórico, com último bloco lido, erros e lacunas explícitos; distinguir amostra de eventos retidos de cobertura histórica.
- [x] Recuperar e vincular transações e recibos aos eventos; tentar rastros internos quando o RPC permitir e registrar indisponibilidade.
- [x] Reconstruir deltas de supply, assets, cap e permissões por bloco/transação, conferindo snapshots históricos e separando questões de semântica do token.
- [x] Conferir invariantes de oferta e balanço em blocos com eventos e entradas de borda no bloco fixado; enviar divergências observadas à quarentena.
- [x] Reproduzir no VPS transações históricas selecionadas sem transmissão à rede, com identidade fixa, timeout e rastros privados; 3/3 replays do YieldVault concluíram.
- [x] Implementar cenários locais em fork Anvil para deposit/mint/withdraw/redeem e sequência deposit→redeem, com snapshots, valores de borda, reversões e pontos `JUMPI` observados; conferir execução no VPS antes de considerar a etapa validada.
- [x] Aceitar relatório de cenários locais no manifesto, conferir identidade e devolver diferenças à quarentena como pedidos de revisão antes de refazer o mapa; não promover automaticamente a M5.
- [x] Reaproveitar uma coleta histórica completa por hash/identidade para reavaliar todos os hotspots sem consultar novamente o RPC, com proveniência explícita.
- [x] Separar pontos `JUMPI` do frame raiz do vault e compará-los com os PCs `JUMPI` do runtime verificado; tratar o denominador como estático, não como prova de alcance.
- [x] Gerar casos de valor zero e um acima de `maxWithdraw`/`maxRedeem` quando disponíveis no fork local, registrando reversão ou conclusão sem inferência automática.
- [x] Resumir resultados de decisão por operação e grupo de entrada a partir dos rastros existentes, sem novo RPC; `local-coverage-v2` no VPS mostra 56/58 resultados já vistos com entradas zero.
- [x] Pesquisar e documentar o desenho da segunda leitura por variável, com AST/layout, SlithIR SSA, source map e fatias de dependência; ver [cadeia de variáveis](VARIABLE-LENS.md).
- [x] Implementar primeiro inventário AST/layout de todas as declarações do source de análise, com leituras, escritas, guardas, atribuições e fatias transitivas por AST ID; execução offline no VPS: 742 declarações e 180 relações. Categorias não presentes no contrato continuam representadas quando declaradas.
- [x] Alinhar PCs observados ao source map verificado com identidade de runtime conferida, sem unir ASTs distintas por nome; 18/50 faixas amplas ficam sem atribuição, 17/50 têm referências candidatas em faixa curta.
- [x] Exportar SSA do Slither localmente e cruzar com o grafo Rust por nome e offset da declaração, preservando origem e confiança: 1.303 operações, 287 relações inferidas, 42 referências posicionadas sem vínculo no VPS.
- [x] Registrar chamadas internas por identificador, argumentos→parâmetros, retornos→resultados e famílias simbólicas de mappings/arrays com expressão da chave e proveniência; sem inferir aliasing ou viabilidade.
- [x] Inventariar variáveis da AST multifile verificada e ligá-las a faixas de PCs do runtime com identidade de compilação e confiança explícita; referência em faixa não prova controle do desvio.
- [x] Implementar seleção automática de cenários locais por limite, ator e mudança de estado com identidade do relatório de variáveis, snapshots e grupos de cobertura.
- [x] Classificar pares de chaves indexadas em igualdade literal, diferença literal, mesmo símbolo ou sobreposição desconhecida; registrar 11 relações possíveis entre funções por estado com proveniência, sem promover símbolo a alias provado.
- [x] Marcar seis chamadas a contrato e 36 limites de assembly na AST do alvo, com classe de efeito e slots literais quando existirem; não atribuir comportamento interno de chamada externa ou assembly dinâmico sem evidência.
- [x] Conferir viabilidade **concreta** por `eth_call` e recibo no mesmo fork; registrar continuidade e reutilização de slots `SLOAD`/`SSTORE` entre etapas locais, além de candidatos à fórmula de mapping por endereço.
- [x] Executar cenários guiados no VPS, conferir hash da cobertura anterior e registrar automaticamente PCs/resultados novos: rodada corrigida com 33 casos, 29 concluídos, quatro reversões de limite, zero falhas RPC/diferenças, ganho de resultados zero; reavaliação com cache manteve 29 zonas/19 quarentenas.

`guided-scenarios-v3/report.json` contém a comparação automática com `local-coverage-v2.json`: 58 resultados anteriores, nenhum resultado ou PC novo. A grade confirma a infraestrutura de ligação, mas não amplia o conjunto de caminhos observados neste alvo. Próxima seleção deve partir de condições não observadas ou apenas vistas em um sentido, com orçamento de estado e viabilidade explícitos.
- [x] Medir PCs estáticos com origem única em função/modifier do bundle verificado e resultados observados, mantendo PCs sem atribuição e faixas amplas explícitos; ampliar sequências pelo feedback dessas lacunas.
- [ ] Demonstrar viabilidade de caminhos não executados, resolver aliases dinâmicos gerais/efeitos externos e obter denominador de ramos **alcançáveis** por função; o denominador atual é apenas PC estático com origem de source única.
- [x] Refazer o mapa inicial com as evidências da quarentena; encaminhar ao M5 apenas hipóteses `UNVERIFIED` vinculadas à identidade do alvo.
- [x] Publicar métricas de cobertura de blocos, eventos, transações, estados e chamadas de borda, com lacunas de rastro/bytecode explícitas.
- [ ] Medir cobertura de caminhos e custo de RPC/IA em campanhas representativas.
- [ ] Validar o ramo positivo completo com alvo autorizado, reprodução Foundry e revisão M8 antes de declarar o scanner forense pronto.

Etapa histórica 2026-10-18: `YieldVault` Sepolia teve 818.846 blocos de logs completos desde o deploy, 5 eventos do vault, 2 transferências do asset, 3 recibos, 1 bloco com eventos conferido e 108 chamadas de borda (103 retornaram). `history_reconciled_trace_gaps` registra a reconciliação dessas evidências e a ausência de rastros internos do RPC. Uma tentativa manual de replay terminou por timeout e não forneceu evidência. O item de sequências em fork permanece aberto.

Após autorização para simular no VPS, o job autônomo `toupera-forensic-replay-20261018.service` reproduziu as três transações históricas selecionadas, com rastros e mensagem de execução bem-sucedida, sem timeout. O resultado está somente no diretório privado do VPS; não houve transação enviada à rede. Isto não fecha o item de sequências inéditas em fork nem a cobertura de caminhos.

Os cenários locais no VPS concluíram 18/18 operações, sem reversão nem diferença de estado após corrigir a formatação de zero; depósito→resgate custou 1 unidade mínima do asset. A reavaliação integral com o relatório local percorreu novamente 818.846 blocos após espera para limite do RPC, preservou 29 zonas/19 quarentenas e não alterou prioridades. Ainda falta uma medida de cobertura por função/ramo com denominador confiável e o ramo positivo M4–M8.

A medição final de cenários registrou 72 posições `JUMPI` no conjunto vault+token, 80 resultados e 8 posições vistas nos dois sentidos. Uma reavaliação com dados históricos salvos conservou 818.846 blocos, 29 zonas e 19 quarentenas, sem consultar o RPC nem mudar prioridades. As contagens de desvio não são percentual de cobertura do vault. Continuam abertos o denominador por função/ramo e a passagem positiva M4–M8.

A grade ampliada final no VPS produziu 24 cenários: 22 concluídos, 2 reversões esperadas nos limites, zero diferenças registradas e zero erros de RPC. No vault, 50 dos 110 PCs `JUMPI` estáticos foram observados, com 58 resultados distintos e 8 PCs em ambos os sentidos. A reavaliação com cache vinculada por hash fez zero chamadas ao RPC e manteve 29 zonas/19 quarentenas. Essa fração estática não é cobertura por função ou por ramo alcançável.


### 1. Autorização e escopo
- [ ] Confirmar propriedade, ambiente de laboratório ou autorização explícita do programa.
- [ ] Registrar limites permitidos e manter análise em Solidity/EVM, vaults, oracles e economia.
- [ ] Não enviar transações ofensivas nem explorar produção.

### 2. Coleta e identificação
- [ ] Registrar endereço/rede ou origem local, versão e hashes disponíveis.
- [ ] Identificar source, bytecode ou ambos; resolver proxy com evidência.
- [ ] Deduplicar artefatos já conhecidos.

### 3. Reconstrução e análise
- [ ] Construir AST ou recuperação de bytecode/CFG conforme a entrada disponível.
- [ ] Mapear funções econômicas, chamadas, modificadores e dependências de storage.
- [ ] Seguir oracle, preço, decimals, fees e rounding até mint/burn e accounting.
- [ ] Verificar previews, operações opostas, primeiro depósito e doações diretas.
- [x] Marcar incertezas; exigir triagem local por identidade, cenários e diferença repetida antes do LLM. Ver `docs/EVIDENCE-FILTER.md`.
- [x] Automatizar rodadas limitadas de valores e sequências a partir do feedback observado, com orçamento e parada por platô; a seleção ainda cobre famílias conhecidas e não resolve condições arbitrárias.

### 4. Hipótese e reprodução
- [ ] Escrever hipótese específica com premissas e contexto mínimo.
- [ ] Criar e executar teste Foundry local autorizado.
- [ ] Guardar resultado reproduzível; descartar ou manter como não verificada se falhar.
- [ ] Confirmar novamente qualquer reprodução positiva.

### 5. Revisão e relatório
- [ ] Validar impacto, condições necessárias e limites do achado.
- [ ] Revisar evidências e regras do programa autorizado com uma pessoa.
- [ ] Redigir relatório com passos de reprodução e sem afirmações além das evidências.
- [ ] Marcar `CONFIRMED` e `REPORT_READY` somente após os gates acima.

## Estados do achado

`UNVERIFIED → REJECTED | REPRODUCED → CONFIRMED → REPORT_READY`

Etapa de triagem 2026-10-19: o modo guiado executou oito variações próximas às escalas observadas, parou pelo platô e gerou 44 casos (40 concluídos, quatro reversões esperadas), zero decisões novas e zero diferenças. O filtro M4/lente/cenários/cobertura/M5 manteve zero chaves elegíveis e plano M6 vazio. Dependências são percorridas até quatro arestas para priorizar variáveis ligadas a usos econômicos; alcançar o limite não prova completude. Artefatos no VPS privado, sem API.

Precisão v2: comparações diretas parâmetro/literal, transições locais de preview, sequências adicionais de ordem/estado, contadores por função, alvos de chamadas e coincidências de slot/layout foram implementados. Continua aberto obter prova de alcance para pontos não observados e resolver efeitos internos/aliases dinâmicos gerais; as novas saídas mantêm esses estados como desconhecidos.


Rodada `precision-v2` no VPS (2026-10-19): serviço `toupera-precision-transition-20261019.service` terminou com status 0. A lente listou 12 comparações diretas em funções auxiliares, nenhuma aplicável diretamente às quatro operações principais. A escolha por mudança de preview fez 68 consultas locais e originou seis casos; a rodada de valores parou após oito casos sem resultado novo no ponto alvo. Quatro sequências de ordem/estado foram executadas antes do próprio platô. Total: 50 casos, 46 concluídos, quatro reversões esperadas, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado permaneceu em 50/110 PCs estáticos, sem resultado novo; 41 PCs têm origem única de source, 23 deles observados. O filtro registrou 25 resumos por função, duas coincidências de slot/layout, quatro acessos indexados sem coincidência, um alvo de chamada observado e zero pedidos de IA. Todas as contagens se limitam ao estado e às entradas da rodada.
