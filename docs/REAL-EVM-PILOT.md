# Piloto EVM real — YieldVault na Sepolia

Executado em 2026-10-18 para verificar as conexões entre M1 e M8 com **leitura pública da Sepolia e análise local**. Alvo: [`YieldVault` em `0xa32f9d514804084839b59972F8b43e616BB4E32b`](https://sepolia.etherscan.io/address/0xa32f9d514804084839b59972F8b43e616BB4E32b#code), chain ID `11155111`. O [repositório público do projeto](https://github.com/guillemustafa-ux/yield-vault) identifica esse deploy. Não houve transação, fork de execução, envio à DeepSeek ou tentativa de exploração.

## Identidade e preparação

- O source verificado no explorer tem 19 arquivos. `src/YieldVault.sol` coincide byte a byte com o arquivo no commit público `005ca9786bd46d6bf765bfab39c392313c606894`; SHA-256 `27d2d4d6181ff2e70afb5a542b179b5195054d1c5c990bbdd6e4eb8c03b2e0bd`.
- A análise fixou o bloco `11944548`, hash `0x069c69928bac2b458e7e253512ae23919065b6fb8c94eb2a4b8f66864ef5d46a`. O runtime retornado por `eth_getCode` tem 9.229 bytes, SHA-256 `ee2630d13cdc5b0f2e8b12f512cdafff283015058f10a96322662441a3cff1f7`.
- O bundle do explorer recompilou sem erros com `solc 0.8.24`, EVM `cancun`, optimizer desativado e `viaIR=false`. O runtime compilado também tem 9.229 bytes. Os 21 bytes divergentes estão **somente** em duas faixas de `immutableReferences` declaradas pelo compilador: offset 2147, 32 bytes, valor `6`; offset 2196, 32 bytes, endereço do asset `0xc10b0e68f21c5fff30d608fe5179ed915a24e423`. Fora dessas faixas, inclusive metadados, os bytes coincidem. O binário oficial do compilador foi conferido contra o hash do manifesto Solidity.
- Para alimentar o M2 atual, os 19 arquivos foram achatados localmente com `forge flatten` em um único arquivo de 3.966 linhas. Essa visão preserva o texto para análise, mas **não** é o source exato implantado: o vínculo com o runtime vem da recompilação do bundle verificado, não do arquivo achatado. Os artefatos e hashes brutos estão em `private-output/real-targets/yield-vault/`, ignorado pelo Git.

## Resultado por etapa

| Etapa | Observação neste alvo |
| --- | --- |
| M1 — IR | 54 funções do source achatado foram representadas com evidência e incertezas. |
| M2 — reconstrução | AST local compilada com `--evm-version cancun`; imports e herança foram convertidos para a visão de um arquivo. O motor ainda não ingere projetos multifile nativamente. |
| M3 — runtime | 5.043 instruções, 611 blocos, 31 selectors, 30 efeitos, 86 fluxos de valor e 299 incertezas. Os 31 selectors do compilador foram correlacionados; nenhum selector ficou sem match. A comparação direta normalizada é falsa por causa dos `immutable`, e a comparação mascarada por suas posições é verdadeira. Rótulos econômicos seguem `Tentative`. |
| M4 — triagem | Zero sinais das regras Rust e zero candidatos. Slither importou 51 sinais; zero passaram o mapeamento econômico seletivo. Isso não é prova de ausência de bugs. |
| M5 — contexto | Pacote privado produzido com fingerprints de source, compilador, runtime, Slither, motor e versão EVM; a repetição idêntica resultou em `cache_hit=true`. Zero slices de candidato. |
| M6 — IA | Plano com zero requests. O comando de revisão emitiu relatório vazio (`provider=none`, zero chamadas/tokens) sem exigir chave ou transmitir código. |
| M7 — Foundry | Planejador produziu zero casos porque não existe hipótese elegível neste alvo; nenhum harness ou teste do alvo foi executado. |
| M8 — evidência | Sem caso M7, não há pacote ou relatório do alvo para finalizar. O gate não autoriza `REPORT_READY`. |

O sinal Slither `incorrect-equality` em `YieldVault.maxMint(address)` aponta `maxAssets == type(uint256).max`. No source, esse valor é a sentinela explícita de cap ilimitado propagada por `maxDeposit`; a igualdade, isoladamente, não estabelece falha econômica. Os nove `divide-before-multiply` aparecem na biblioteca `Math`, não como exploração demonstrada do vault. Os demais alertas não foram auditados individualmente. O importador M4 só transforma detectores permitidos em candidatos quando há função e span inequívocos; os 51 registros continuam disponíveis para revisão.

## Interpretação e próximo gate

Este piloto valida entrada real, identidade source/runtime com `immutable`, reconstrução e **saída vazia segura** de M4→M7. Ele não valida o ramo positivo M4→M5→M6→M7→M8. O [piloto sintético M7](M7-PILOT.md) exercitou reprodução, rejeição e empacotamento por casos manuais, mas sua divergência não foi selecionada por M4. Antes de declarar o caminho inteiro conectado, calibrar M4 nessa fixture conhecida e repetir a passagem automática com um candidato autorizado, um harness específico revisado e evidência M8. Não inferir segurança do YieldVault a partir de zero candidatos.

## Extensão forense automática

O [ciclo Rust de mapa e quarentena](FORENSIC-QUARANTINE.md) foi executado no mesmo bloco. O RPC primário não serviu o estado fixado na repetição; o fallback `sepolia.gateway.tenderly.co` só foi aceito após conferir chain ID, hash do bloco e runtime exato. O mapa teve 29 zonas e 19 casos de quarentena; 78 `eth_call` de borda e 15 relações observadas foram registrados. As funções `deposit`, `mint`, `withdraw` e `redeem` ficaram no topo porque as chamadas herdadas chegam a `_totalSupply`, `_mint`/`_burn` e ao asset externo. A quarentena rastreou também `depositCap` e `_asset`, com leitores, escritores, source spans, fluxos, PCs e limitações por caso.

No bloco fixado, `totalAssets()` e `asset.balanceOf(vault)` retornaram 1.100.000.000 unidades; `totalSupply()` retornou 1.000.000.000.000.000 shares; `depositCap()` retornou 1.000.000.000.000; `maxDeposit()` retornou 998.900.000.000. A primeira passagem fez 78 consultas e examinou apenas os 20.000 blocos finais. Esse resultado parcial foi substituído por uma segunda passagem histórica de leitura, sem transações enviadas e sem teste Foundry no alvo.

Na segunda passagem, o motor escolheu o RPC com histórico mais antigo após validar a identidade fixa. Ele percorreu inclusivamente os 818.846 blocos de 11125703 a 11944548 para logs do vault e transferências do asset. Encontrou 5 eventos do vault em 3 transações e 2 transferências do asset para o vault, somando 1.100.000.000 unidades de entrada e zero de saída. O replay dos eventos de shares calculou emissão de 1.000.000.000.000.000 e queima zero, compatíveis com `totalSupply`. O cap, o owner e o balanço do asset conferiram no único bloco com eventos. Os 3 recibos foram inspecionados. Das 108 chamadas de borda, 103 retornaram e 5 registraram erro/reversão. Nenhuma anomalia foi observada nessas comparações; M5/M6 permaneceram sem candidato ou pedido.

O relatório marca `coverage.evidence_gate=history_reconciled_trace_gaps`: a cronologia de logs, recibos e estado histórico selecionado foi reconciliada, porém nenhum rastro interno estava disponível (`debug_traceTransaction` recebeu HTTP 429). A inspeção de estado se limitou ao único bloco com eventos neste contrato. Há 299 itens de bytecode não resolvidos e as chamadas de borda não cobrem todos os ramos ou sequências que alteram estado. Uma tentativa manual de replay local terminou por timeout sem rastro útil; ela não foi usada como evidência. Esses limites impedem tratar a ausência de anomalia como aprovação de segurança.

## Replay autônomo no VPS

Após autorização posterior para simular **no VPS**, um job `systemd-run` executado como `toupera` usou `cast run --no-bal` nas três transações históricas selecionadas. O job verificou chain ID, hash do bloco fixado e SHA-256 do runtime antes de começar. Em 2026-10-19 UTC, os três replays produziram rastros locais e terminaram com `Transaction successfully executed`: deploy (gas 2.170.886), distribuição de yield (gas 46.492) e depósito (gas 115.223). Foram 3/3 rastros, zero timeout e zero falha do comando. O resumo, os hashes SHA-256 dos rastros e os rastros completos estão apenas em `/srv/toupera-agent/private-output/real-targets/yield-vault/replay-vps-20261018/`. Nenhuma transação foi transmitida à Sepolia. Esse replay confirma que o servidor consegue reproduzir essas transações publicadas; não mede caminhos inéditos ou cobertura de ramos, e não transforma a ausência de anomalia em prova de segurança.

## Cenários locais e reavaliação

O motor Rust executou 18 operações em um Anvil de loopback no VPS: quatro entradas isoladas para cada uma de `deposit`, `mint`, `withdraw` e `redeem`, mais depósito→resgate sem restaurar o estado entre etapas. A execução final registrou zero reversões, falhas de RPC ou diferenças entre previews e mudanças de estado. O round trip devolveu 1 unidade mínima do asset a menos. Rastros locais mostraram 72 posições `(profundidade, PC)` de `JUMPI`, 80 resultados distintos e 8 posições observadas nos dois sentidos. A contagem inclui chamadas ao token e não representa percentual de cobertura do vault.

Uma passagem completa do mapa com a segunda execução local percorreu novamente os 818.846 blocos e manteve 29 zonas, 19 quarentenas e zero alteração de prioridade. O RPC exigiu espera após HTTP 429; a primeira tentativa foi marcada `partial`, e a segunda concluiu. Para a execução final dos cenários, o modo `--cached-reassessment` vinculou seu relatório ao SHA-256 exato de uma coleta histórica completa e reavaliou todos os hotspots salvos com **zero chamadas ao RPC**. Também não houve nova diferença que justificasse encaminhamento à IA. Os relatórios permanecem privados no VPS.
