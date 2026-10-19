# Segunda leitura: cadeia de variáveis

## Motivo e evidência atual

O mapa atual parte de funções e zonas prioritárias. Uma segunda leitura deve partir de **cada variável declarada**, seguir o valor que entra, suas transformações, condições, leituras, escritas e efeitos externos, e voltar das consequências aos valores de origem. Isso inclui variáveis locais e parâmetros, estado persistente e transitório, `immutable`/constantes, membros de estruturas, elementos de arrays e chaves de mappings. A identidade de uma variável deve incluir contrato, AST ID, função/escopo, tipo e localização; nome textual sozinho não basta.

No YieldVault Sepolia, o relatório local `local-coverage-v2.json` (somente no VPS) resumiu 24 cenários e 58 resultados de decisão observados no frame raiz. Quatro chamadas com entrada zero já somaram 56 resultados. Dezesseis casos positivos comuns não acrescentaram resultado além desses 56; dois casos acima do limite acrescentaram os outros dois. Isso orienta os próximos cenários para **estado, ator, ordem e limiares derivados do código**, sem inferir que os 60 PCs `JUMPI` estáticos não observados sejam todos alcançáveis. Atribuição por operação hoje usa o nome da chamada principal, não uma correspondência comprovada de cada PC à função Solidity.

## Base técnica pesquisada

- O compilador Solidity fornece `storageLayout` com AST ID, slot, offset e tipo; mappings e arrays exigem cálculo próprio e slots podem conter várias variáveis. `constant`, `immutable` e storage transitório exigem categorias separadas. [Layout oficial do Solidity](https://docs.soliditylang.org/en/latest/internals/layout_in_storage.html).
- SlithIR normaliza instruções e permite seguir leituras, escritas e dependências de dados; sua API disponibiliza dependência e contaminação em SSA. É uma fonte complementar para parâmetros e temporários que `storageLayout` não descreve. [SlithIR](https://github.com/crytic/slither/wiki/SlithIR), [API de dependências](https://crytic.github.io/slither/slither/analyses/data_dependency/data_dependency.html).
- O mapa de origem do solc associa instruções EVM a trechos do source. A ligação PC→variável deve conservar a confiança e não converter uma associação ambígua em identidade confirmada. [Source mappings do Solidity](https://docs.soliditylang.org/en/latest/internals/source_mappings.html).
- SAILFISH usa um grafo de dependência de storage e refina caminhos candidatos com resumos de valores; a ideia útil aqui é separar **onde uma variável influencia** de **qual combinação de estados é viável**. [Artigo original](https://arxiv.org/abs/2104.08638).
- Quando source não está disponível ou a identidade source/runtime falha, Gigahorse pode elevar bytecode a TAC e fornecer fatos de fluxo e memória. Nomes e tipos recuperados nessa via permanecem tentativos. [Gigahorse](https://github.com/nevillegrech/gigahorse-toolchain).

## Desenho para o motor Rust

1. **Inventário completo e identidade.** Ler AST, layouts do solc e IR atual; registrar também variáveis sem leitura/escrita recuperada. Separar `storage`, transitório, `immutable`, constante, calldata, memória e stack. Desdobrar estruturas em membros e mappings em famílias parametrizadas por chave, sem fingir que todas as chaves possíveis foram observadas.
2. **Grafo tipado.** Criar nós `valor`, `operação`, `condição`, `leitura/escrita`, `chamada`, `evento` e `PC`; arestas `origina`, `transforma`, `controla`, `grava`, `observa`. Cada aresta guarda source span ou PC, ferramenta de origem, contrato/runtime e confiança. Cruzar AST/SlithIR/SSA com layout e bytecode; marcar desacordos.
3. **Fatia nos dois sentidos.** Para cada variável, caminhar para trás até entradas, oracles e estado anterior; caminhar para frente até guards, transfers, mint/burn e retornos. Incluir dependências entre funções e entre transações, com limites explícitos para chamada externa, assembly, proxy e delegatecall.
4. **Limiar e estado.** Extrair comparações, mudanças de sinal, divisões/arredondamento, limites máximos, permissões e dependências de ordem. Gerar pares de estados e entradas em torno desses pontos e escolher primeiro os que prometem observar resultados ainda não vistos. Executar apenas no fork isolado do VPS, conferindo identidade e registrando delta de cobertura.
5. **Reavaliação.** Se uma cadeia mostrar diferença concreta entre previsão, estado e efeito observado, encaminhar a variável e sua fatia à quarentena; depois reconstruir o mapa global. A IA recebe somente a fatia ambígua e evidências após os filtros determinísticos.

## Saída e critérios de progresso

O novo relatório deve listar total de declarações por categoria, quantas têm cadeia recuperada, arestas sem correspondência source/runtime, variáveis com leitura sem escritor conhecido, locais/chaves não resolvidos, condições ligadas a cada variável e resultados novos por cenário. A pontuação de prioridade deve explicar cada parcela, como proximidade de transferência/mint/burn, controle por usuário/oracle, arredondamento e lacuna observada; pontuação **não** é probabilidade de falha.

## Primeiro recorte implementado no VPS

`security-engine --variable-lens <manifesto.json> <relatório-forense.json> <local-coverage.json>` roda sem RPC, fork ou IA. Valida SHA-256 do source, do runtime, da saída verificada do compilador e do relatório base; confere chain, endereço, bloco e hash do relatório local. Recompila o source de análise com AST e `storageLayout`, enumera declarações por AST ID, tipo, escopo, papel e categoria, registra leituras/escritas/guardas/argumentos, constrói relações de atribuição e inicialização e calcula fatias transitivas nos dois sentidos. O relatório privado `variable-lens-v4.json` foi produzido pelo binário Rust no VPS.

No YieldVault, a visão achatada gerou **742 declarações** (7 de estado persistente, 2 `immutable`, 10 constantes e outras categorias), **180 relações sintáticas** e 346 declarações sem uso recuperado no source. Isso inclui parâmetros de interfaces e não é uma medida de execução. `depositCap` aparece no slot 6 do layout da compilação achatada: `cap_` e `newCap` chegam às escritas no construtor e em `setDepositCap`; uma leitura segue para `cap` em `maxDeposit`. A posição do slot vem da compilação do source de análise e **não foi ligada diretamente ao slot do runtime implantado**.

O modo também lê o source map do bundle multifile verificado, alinhando os PCs do compilador aos PCs do runtime fixado. Dos 50 `JUMPI` observados no frame raiz, 18 têm faixas de origem amplas e não recebem nome de variável; 32 têm faixa menor e 17 delas contêm referências candidatas a variáveis. Esses vínculos permanecem `tentative_source_range`: uma faixa curta ainda pode incluir uma variável que não controla aquele desvio. IDs da AST verificada não são unidos aos IDs da AST achatada por semelhança de nome.

**Próximas integrações ainda abertas:** exportar e cruzar dependências SSA do Slither; resolver chaves/índices e fluxo entre funções com mais precisão; vincular variáveis do bundle verificado ao runtime sem depender da transformação achatada; gerar e executar novas variações de estado/ator/ordem no VPS e medir resultados novos. A pontuação atual serve para ordenar revisão, não para indicar probabilidade de diferença.
