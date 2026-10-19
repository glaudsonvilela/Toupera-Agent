# Mapa forense e motor de quarentena

O comando `--forensic-probe` executa um ciclo local e reproduzível para um runtime EVM verificado. A entrada é um manifesto `forensic-input-v1` com source de análise, bundle compilado, runtime, endereço, chain ID, bloco fixado, RPC e Slither opcional. O JSON resultante usa `forensic-map-v1`. Os artefatos do alvo devem ficar em `private-output/`, fora do Git.

```sh
./target/release/security-engine --forensic-probe private-output/alvo/forensic-input.json > private-output/alvo/forensic-report.json
./target/release/security-engine --context-package private-output/alvo/flattened.sol private-output/alvo/cache ./solc --bytecode private-output/alvo/runtime.hex --forensic private-output/alvo/forensic-report.json > private-output/alvo/context.json
```

## Ciclo automático

1. **Motor 1 — mapa inicial.** Compila o source local, reconstrói funções, chamadas, dependências de storage e fluxos de valor; correlaciona selectors e PCs do bytecode e importa sinais Slither. Só aceita RPC que responda ao chain ID, bloco e runtime exatos. A prioridade de cada zona soma razões explícitas como mutação de shares/assets, ramificações, chamadas externas, aritmética e incerteza. É uma ordem de investigação, **não** uma probabilidade de bug.
2. **Motor 2 — quarentena.** Para até 24 zonas com prioridade ao menos 25, seleciona a variável mais sensível, reúne leitores, escritores, spans, fluxos, efeitos e sinks em PC, estado no bloco fixado, chamadas `eth_call` nos limites e eventos disponíveis. Registra reversões e cobertura parcial; uma chamada que reverte, sozinha, não constitui anomalia econômica. Não envia transações.
3. **Motor 1 — reavaliação.** Reconstrói o mapa inteiro a partir da fonte e do runtime e incorpora apenas anomalias concretas da quarentena. Amostras sem anomalia não reduzem prioridade nem declaram segurança. A execução tem uma rodada, evitando realimentação infinita.
4. **Encaminhamento.** Apenas `anomaly_observed`, com hash do source e runtime compatíveis, pode entrar no M5 como candidato `UNVERIFIED`. O cache M5 inclui o hash do relatório forense. M6 ainda exige seleção e autorização explícita para envio ao DeepSeek; M7 continua dependente de harness específico e revisão.

O relatório separa `hotspots`, `quarantine_cases`, `reassessment`, `onchain_state`, `boundary_probes`, `observed_relations`, `event_history` e `unresolved`. O fallback de RPC é aceito somente após as mesmas conferências de identidade. A busca de eventos tem janela máxima de 20.000 blocos e pode ser parcial mesmo quando o estado histórico está disponível. Saltos dinâmicos, composição de source achatado e fluxos externos desconhecidos permanecem incertezas. O mecanismo não executa fuzzing, simbólico ou todas as trajetórias e não calcula risco estatístico nem encontra todo tipo de DoS ou revert.

## Referência Sepolia

No `YieldVault` do [piloto real](REAL-EVM-PILOT.md), bloco 11944548, o motor emitiu 29 zonas, 19 casos de quarentena e 78 chamadas de borda. `deposit`, `mint`, `withdraw` e `redeem` lideraram a prioridade após seguir chamadas herdadas até supply e token. Nenhuma anomalia foi observada nas amostras; M5 manteve zero candidatos e M6 zero pedidos. Os eventos foram lidos só na janela final de 20.000 blocos, portanto a conclusão é limitada.

**Cobertura:** o bundle verificado contém o source completo, e a reconstrução representou 54 funções. O mapa pontuou 29 zonas, das quais 19 receberam quarentena. As chamadas `eth_call` amostraram entradas de leitura e não percorreram todos os ramos ou sequências de transações. A janela de 20.000 blocos é aproximadamente 2,4% dos 818.845 blocos entre o deploy identificado e o bloco fixado. Assim, a parcialidade se refere à investigação de caminhos e eventos, não à coleta do source.
