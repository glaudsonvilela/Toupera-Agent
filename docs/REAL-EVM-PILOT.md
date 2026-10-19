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

No bloco fixado, `totalAssets()` e `asset.balanceOf(vault)` retornaram 1.100.000.000 unidades; `totalSupply()` retornou 1.000.000.000.000.000 shares; `depositCap()` retornou 1.000.000.000.000; `maxDeposit()` retornou 998.900.000.000. As amostras não mostraram anomalia; 9 casos ficaram `sampled_no_anomaly_observed` e 10 em `static_crosscheck_only`. O motor 1 refez o mapa e conservou as prioridades. M5 continuou com zero candidatos, M6 com zero requests. A busca de eventos só examinou os 20.000 blocos finais, embora o deploy tenha sido identificado no bloco 11125703; ausência de eventos nessa janela não significa ausência histórica. Não houve transações ou teste Foundry neste alvo.

O source verificado foi coletado integralmente; a cobertura parcial se refere à investigação. Das 54 funções reconstruídas, 29 formaram zonas de prioridade e 19 foram aprofundadas. As 78 consultas de leitura não cobrem todos os ramos ou sequências que alteram estado. A janela de eventos representa aproximadamente 2,4% dos 818.845 blocos entre o deploy e o bloco de análise.
