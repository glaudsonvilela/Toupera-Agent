# Plano local de leitura

O comando `--forensic-plan` prepara uma próxima rodada a partir de cinco arquivos da mesma identidade: manifesto, mapa fixo, lente, cenários e cobertura. Ele confere hashes do runtime, compilação, relatório e cobertura antes de usar os dados. O resultado é um JSON privado. O modo `--guided-scenarios` pode receber esse plano como último argumento e executa apenas entradas elegíveis no Anvil filho. Se o plano registrar parada, uma nova rodada é recusada antes de consultar RPC.

## O que foi ligado

1. **Decisões e variáveis.** O plano lê as condições simples da AST verificada e cruza faixas curtas do source map com PCs `JUMPI`. Se a faixa não permite ligação única, deixa o ponto sem nome. Em paralelo, usa o fluxo abstrato do bytecode para indicar classes de origem como calldata, storage e retorno externo. Classe de origem não identifica a variável exata.
2. **Valores de condição.** Para comparação direta de um parâmetro inteiro com um literal, calcula valores que tornam a expressão verdadeira e falsa, respeitando os limites de `uint256`. A direção do salto EVM continua desconhecida até observar a execução. Expressões compostas e condições dependentes de estado não recebem solução numérica inventada.
3. **Ordem de estado.** Usa slots escritos e lidos em execuções isoladas anteriores para propor até oito pares de operações. Cada par roda a partir de snapshot no Anvil local. Coincidência de slot sugere uma ordem para observar; não prova dependência causal.
4. **Relações numéricas.** Registra deltas esperados e observados por operação, compara previews em entradas isoladas crescentes e verifica se a diferença entre `totalAssets()` e saldo do asset mudou. Um sinal é uma observação com premissas explícitas, não uma conclusão automática.
5. **Medição.** `--source-benchmark` mede condições simples em arquivos de laboratório. `tools/measure_local.py` usa Anvil em `127.0.0.1` para comparar uma sequência de oito valores com uma escolha curta de zero, um e limites literais do primeiro parâmetro. O programa só aceita saída em `private-output/`. Não usa contratos externos nem envia transações à rede.

## Rodada no contrato fixado

O plano leu 17 condições simples do bundle verificado. Entre 36 PCs de origem de função única que ainda tinham zero ou um resultado, nenhum recebeu ligação estreita e única com uma dessas condições; os 36 continuam sem solução direta. O fluxo abstrato encontrou alguma classe de origem para 46 de 102 PCs ainda com zero ou um resultado no runtime inteiro. Isso orienta o tipo de próxima entrada, mas não identifica 46 variáveis do source.

Foram propostos oito pares de ordem a partir de slots observados. Quatro foram executados antes do platô. A rodada completa teve 52 casos, 48 concluídos, quatro reversões, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado continuou em 50 de 110 PCs `JUMPI` estáticos e 58 resultados distintos; nenhum novo resultado foi registrado frente à cobertura anterior. A nova leitura gerou 240 comparações de delta, 16 pares de preview e 48 diferenças contábeis antes/depois: zero divergências, zero reversões de ordenação e zero mudanças da diferença contábil nessa rodada. O próximo plano marcou `plateau_without_distinct_executable_probe`; o comando de cenário recusou uma rodada repetida antes do RPC. O filtro permaneceu sem candidato e o plano de IA vazio.

## Medição em três arquivos próprios

A compilação local encontrou, respectivamente, 2, 2 e 2 condições simples em `RoundingVaultM7Fixture`, `VaultFlowFixture` e `VaultWithOracle`; somente o primeiro tinha duas comparações diretamente executáveis a partir do primeiro parâmetro. Isso mede extração de condições, não caminhos percorridos.

A comparação no Anvil usou três outros cenários próprios. Cada entrada começou do mesmo snapshot. Os números abaixo contam resultados `(PC, direção)` distintos, chamadas RPC feitas pelo medidor e apenas uma execução por conjunto:

| Cenário | Entradas sequenciais 1–8 | Entradas escolhidas automaticamente | Resultados sequenciais → escolhidos | RPC sequencial → escolhido |
|---|---:|---:|---:|---:|
| `RoundingVaultM7Fixture.withdraw` | 8 | 4 (`0,1,7,8`) | 15 → 16 | 64 → 32 |
| `ModifierFlowFixture.deposit` | 8 | 2 (`0,1`) | 9 → 12 | 48 → 12 |
| `VaultLoopFixture.sumUntil` | 8 | 2 (`0,1`) | 13 → 11 | 48 → 12 |

O medidor observou a diferença conhecida entre preview e execução no primeiro cenário em entradas dos dois conjuntos. No caso de loop, a escolha curta perdeu dois resultados: a regra atual ainda precisa entender condições entre variáveis e progressão de iterações. As contagens e tempos de uma rodada não provam ganho geral. A medição compara **políticas de entrada**; não isola todo o custo do chip Rust nem substitui campanhas com estados variados.

Os JSONs detalhados ficam somente em `/srv/toupera-agent/private-output/real-targets/yield-vault/precision-plan-v1/` e `precision-plan-v2/`. O código, este documento e o resumo de texto podem ser publicados no Git privado. Próximos limites: faixas amplas do source map, expressões compostas, entradas de loop guiadas por estado e medição dinâmica em mais contratos de escopo autorizado.
