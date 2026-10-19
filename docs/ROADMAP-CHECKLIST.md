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

Chip adaptativo: `--guided-scenarios` aceita um relatório anterior da mesma identidade e cuja cobertura corresponde ao hash do arquivo. O motor escolhe variações por histórico de operação/valor e classe de comportamento, evita entradas repetidas, registra orçamento e motivo de parada. Ver `docs/ADAPTIVE-CHIP.md`. Ainda falta medir ganho de cobertura em campanhas com alvos variados e demonstrar o ramo positivo completo M4–M8.


Rodada `precision-v2` no VPS (2026-10-19): serviço `toupera-precision-transition-20261019.service` terminou com status 0. A lente listou 12 comparações diretas em funções auxiliares, nenhuma aplicável diretamente às quatro operações principais. A escolha por mudança de preview fez 68 consultas locais e originou seis casos; a rodada de valores parou após oito casos sem resultado novo no ponto alvo. Quatro sequências de ordem/estado foram executadas antes do próprio platô. Total: 50 casos, 46 concluídos, quatro reversões esperadas, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado permaneceu em 50/110 PCs estáticos, sem resultado novo; 41 PCs têm origem única de source, 23 deles observados. O filtro registrou 25 resumos por função, duas coincidências de slot/layout, quatro acessos indexados sem coincidência, um alvo de chamada observado e zero pedidos de IA. Todas as contagens se limitam ao estado e às entradas da rodada.

Rodada do chip no VPS: oito observações anteriores carregadas; 20 entradas repetidas e quatro vizinhos equivalentes evitados; 61 opções ordenadas; oito casos adicionais sem classe/resultado novo, então parada por platô. 50 casos totais, 46 concluídos, quatro reversões, zero falhas ou diferenças de pré-checagem; 50/110 PCs estáticos e 58 resultados observados, sem aumento. Filtro sem candidato e plano de IA vazio. Serviço concluído com status 0. A política está operacional, mas o ganho de alcance ainda precisa de outros alvos/estados.

Plano local concluído nesta etapa: [x] registrar classes de origem no bytecode e ligações estreitas de source, com pontos não resolvidos explícitos; [x] calcular pares verdadeiro/falso para comparação direta de primeiro parâmetro com literal; [x] propor e executar ordens por slots escritos/lidos em snapshots; [x] medir deltas, previews e diferença contábil; [x] comparar políticas de entrada em três cenários próprios no Anvil e rodar o plano no contrato fixado. Ver `docs/SIMPLE-PLAN.md`. Continuam abertos resolver expressões compostas e condições de loop sem perda de resultados, além de ampliar a medição dinâmica em outros contratos autorizados.

## Painel wCOP no VPS
- [x] Formulário de endereço e terminal de progresso no site.
- [x] API autenticada e limitada a um contrato/trabalho por vez.
- [x] Criar `smart.md` na raiz e histórico privado por trabalho.
- [x] Fixar rede, bloco, hash, runtime e implementação; executar Rust no VPS.
- [x] Criar `result_smart.md` e consolidar estados reais das dez ferramentas em `end_smart_result.md`.
- [x] Exibir o Markdown final no frontend autenticado.
- [ ] Projetar propriedades específicas e harness local para Foundry, Echidna/Medusa e Halmos.
- [ ] Revisar sinais brutos antes de qualquer conclusão ou comunicação ao programa.
- [x] Execução real do wCOP no VPS: proxy, implementação, Rust, Slither e Mythril com resultado e limitações explícitas.
- [x] Remover configuração de deploy da raiz pública e manter hash ativo apenas no Caddy do VPS.
- [x] Validar o fluxo final pela API autenticada e recuperar trabalhos interrompidos.
- [x] Integrar três regras Foundry em fork local do bloco fixado para o wCOP, sem transmissão de transações.
- [x] Validar integração automática de Slither, Mythril e Foundry no wCOP, com painel final autenticado.


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

## Arquitetura de confiança (2026-10-19)

- [x] Desenhar arquitetura, fronteiras, interfaces, estados e critérios antes da implementação. Ver docs/ARQUITETURA-CONFIANCA-2026-10-19.md.
- [x] Implementar manifesto e limites de entrada.
- [x] Implementar verificação de AST e fallback para bytecode.
- [x] Implementar decisão do guardião Rust por afirmação.
- [x] Ligar decisão ao relatório e ao painel; a revisão permanece humana.
- [x] Medir o fluxo completo no wCOP com evidências privadas no VPS.

- [x] Preparar manifesto privado e saída AST com limites de entrada; ligação final depende do CLI Rust.

### Entrega do guardião

- Trabalho integrado 3932c3f9ca4e4faf: bloco 26228121, 34 ASTs, identidade válida e vínculo com ressalva para campos imutáveis. Cópias privadas confirmaram rebaixamento de AST alterada e fallback sem fonte.
- [x] Integrar decision.json ao pacote M8, com opção --guard-job, hashes e verificação antes da finalização.
- [x] Isolar o serviço por permissões, arquivos de trabalho separados e opções systemd. Falta isolamento por processo.
- [ ] Medir a política em mais contratos autorizados.
- Detalhes e limites: docs/GUARDIAO-EVIDENCIAS-2026-10-19.md.

- [x] Repetir Foundry apenas em erro transitório do RPC, guardando logs das tentativas no VPS.

- Rodada de fechamento `d59cb58cee084b10`: serviço restrito ativo, guardião vinculado e 18/18 cenários Foundry locais. Ver `docs/ISOLAMENTO-E-VINCULO-M8-2026-10-19.md`.

- [x] Separar cada ferramenta por namespaces de montagem/PID/IPC/host, com um único trabalho visível e gravável; Mythril usa cache privado. A rede permanece compartilhada para leituras.

- Rodada final `3fdf10704a64482c`: trabalho privado isolado, seis ferramentas aplicáveis concluídas, 18/18 cenários Foundry e vínculo M8 em `PENDING_REPRODUCTION`. Resultado documentado em `docs/ISOLAMENTO-E-VINCULO-M8-2026-10-19.md`.

## Ciclo adaptativo local

- [x] Planejador Rust orientado por feedback do runtime e das tentativas anteriores.
- [x] Executor local gera combinações de operação/conta/valor e registra diferenças para revisão.
- [x] Ligar o ciclo ao trabalho do painel e ao manifesto do guardião.
- [x] Conferir a rodada completa no VPS, medir ganhos e ajustar a confirmação das operações locais e a passagem de valores grandes. Trabalho `fe5aa294573f4571`: 18 escolhas, 7 operações locais aplicadas, +34 PCs, +2 transições, zero diferenças registradas; ver `docs/CICLO-ADAPTATIVO-2026-10-19.md`.
- [ ] Ampliar a gramática e os estados explorados antes de falar em cobertura mais ampla.

## Andar pelo mapa — etapa 2

- [x] Usar saídas adjacentes ainda não vistas na escolha Rust.
- [x] Montar sequências curtas e desfazê-las na cópia local.
- [x] Ampliar valores próximos e opções de autorização e assinatura.
- [x] Conferir o fluxo completo no VPS: trabalho `f7c70e446f134e78`, +578 PCs, +50 ligações, +38 blocos, 11 passagens para blocos novos, zero diferenças registradas.
- [ ] Medir mais estados e ligar os caminhos do bytecode a uma nova medição de ramificações da fonte.

## Rodada completa wCOP — 7011fcd042db41e5

- [x] Gerar novamente os três arquivos Markdown no VPS para um bloco fixado.
- [x] Ler Rust, histórico, cobertura, sequências e saídas das dez ferramentas.
- [x] Pontuar individualmente 236 registros, 40 escolhas, 30 dúvidas do Rust e 114 sinais do Slither em arquivo novo no Git privado.
- [x] Distinguir prioridades de revisão de falhas confirmadas; nenhuma falha confirmada nesta rodada.
- [ ] Ampliar estados e ligar novas saídas do bytecode a uma medição atualizada dos ramos da fonte.

### Continuação: mapa de linhas e permissões (2026-10-19)

- [x] Ligar PCs das duas leituras locais a arquivo/linha pelo mapa do compilador, com vínculo de runtime e campos imutáveis.
- [x] Registrar posições sem linha, dados depois do mapa e ligações observadas fora do mapa estático.
- [x] Integrar o mapa ao trabalho do painel e à conferência de identidade do Rust.
- [x] Refazer a sequência de permissões por eventos e comparar as contas vistas com o estado no bloco.
- [ ] Demonstrar que todas as contas possíveis foram descobertas; atualizações e gravações sem evento ainda limitam essa afirmação.
- [ ] Separar código e dados embutidos no denominador de posições e medir caminhos alcançáveis por função.

Ver `docs/WCOP-MAPA-LINHAS-2026-10-19.md`.

- [x] Incluir arquivos internos do compilador no mapa de linhas e distingui-los da fonte original; 2504/3048 posições observadas com local após esta etapa.

- [x] Ordenar ligações e entradas no Rust e usar a prioridade no ciclo local.
- [x] Acrescentar duas combinações locais com preparação de dois passos e retorno decrescente para repetições.

- [x] Ligar registros de troca a blocos fixos e conferir código, posição, oferta e contas vistas em cada versão.

- [x] Criar filtro Rust de revisão: só permitir contexto compacto com diferença local repetida e vínculos observados.

- [x] Concluir uma rodada integrada após mapa, ordem Rust, linha de versões e filtro; ver `docs/WCOP-RODADA-7d78331aa23a4592.md`.
- [ ] Conferir estado da troca em bloco antigo por provedor que disponibilize esse histórico; atual provedor respondeu 403.
- [ ] Expandir leitura para condições ainda não observadas; 38 escolhas pararam por platô, sem diferença local registrada.

## Continuação do mapa — limite do compilador (2026-10-19)

- [x] Conferir no Rust o fim do mapa contra runtime e leituras.
- [x] Separar os PCs posteriores do ganho que orienta a próxima escolha, preservando as contagens totais.
- [x] Conferir offline no VPS: 37 PCs após o mapa, zero observados no trabalho `7d78331aa23a4592`.
- [x] Classificar os 536 PCs observados sem linha: todos têm `file_id = -1` no mapa do compilador; ver `docs/WCOP-FORENSE-MAPA-2026-10-19.md`.
- [x] Cruzar, somente por arquivo e linha, PCs observados e registros de ramo LCOV, sem equiparar as duas coberturas.
- [x] Classificar o opcode terminal das 74 ligações fora do grafo: todas são `JUMP`; 68 têm marca de saída de função e seis de salto comum. Todas as 143 ligações observadas com `JUMPI` terminal constam no grafo estático.
- [x] Repetir os casos locais com condição, destino e próximo PC de cada `JUMPI` por caso; 36 casos iniciais e 38 escolhas do Rust reproduziram os resumos salvos, e 65 candidatos restantes foram medidos. Ver `resumo.md`.
- [x] Executar 22 casos dirigidos no Anvil com identidade fixada: 155/169 PCs `JUMPI` estáticos observados, 54 em ambos os sentidos, 101 em um; `permit` válido e repetição testados em snapshot. Ver `docs/WCOP-DESVIOS-DIRIGIDOS-2026-10-19.md`.
- [ ] Ligar os resultados de `JUMPI` a um identificador de ramo LCOV com alinhamento semântico; 14 PCs `JUMPI` seguem não observados após os 22 casos dirigidos; a possibilidade de alcançá-los no proxy fixado não foi demonstrada.
- [x] Medir uma nova rodada local: trabalho `92273047cbf7439c`, 38 escolhas e zero PCs após o mapa; total observado igual ao trabalho anterior. Não há ganho de cobertura demonstrado.

Ver `docs/WCOP-FOCO-MAPA-2026-10-19.md`.
