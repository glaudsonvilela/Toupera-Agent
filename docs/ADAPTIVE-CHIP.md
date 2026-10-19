# Chip adaptativo de cenários

O módulo `adaptive_chip.rs` seleciona entradas para a rodada local de `--guided-scenarios`. Ele é uma política determinística em Rust: não treina modelo, não consulta IA e não decide que uma diferença é um achado.

## Ciclo

1. Recebe o mapa, a cobertura anterior e, opcionalmente, o relatório da rodada anterior. Confere contrato, bloco, runtime, hash do mapa e que a cobertura foi produzida por esse relatório.
2. Guarda a combinação de operação e valor já usada no estado inicial. Reduz a prioridade de vizinhos quando duas observações próximas têm a mesma classe de comportamento.
3. Forma valores ao redor de escalas conhecidas e entre pares que apresentaram classes diferentes. Ordena candidatos com preferência por operação associada a ponto ainda pouco observado, intervalo de transição e região distante do histórico.
4. Após cada execução no Anvil local, resume status, pré-checagem, direção das mudanças de estado e resultados dos desvios do contrato. Valores numéricos exatos não tornam, sozinhos, duas execuções classes distintas.
5. Interrompe as variações após oito casos sem classe nova nem resultado novo em ponto alvo, ou no limite de 16 casos. Guarda motivo, contadores e até 256 observações no relatório privado. Uma nova rodada pode receber esse relatório como memória.

Uso no VPS, com os arquivos privados da mesma identidade:

```text
security-engine --guided-scenarios <mapa.json> <lente-anterior.json> <cobertura-anterior.json> <rpc-do-fork> <anvil> [cenarios-anteriores.json]
```

As transações de cenário vão somente ao Anvil filho em `127.0.0.1`; o RPC da rede serve para consultar o bloco fixado e criar o fork. O chip não interpreta por si só a viabilidade de um ponto não observado. Classes iguais não provam equivalência de caminhos, e a parada por platô não prova ausência de diferenças. A memória é limitada e o relatório marca quando a capacidade é atingida. O filtro de evidências continua sendo a etapa separada que decide se existe contexto elegível para IA.

Arquivos brutos de rodadas ficam apenas em `private-output/` no VPS. O resumo público de uma rodada deve conter somente contagens agregadas e limites da interpretação.

## Medição inicial no VPS (2026-10-19)

O serviço `toupera-adaptive-chip-20261019.service` terminou com status 0. A memória recuperou oito entradas da rodada anterior, deixou de repetir 20 combinações já vistas e afastou quatro variações próximas de comportamento igual. Ordenou 61 candidatos; executou oito casos de feedback e parou por oito sem nova classe nem resultado no ponto alvo. A rodada completa registrou 50 casos, 46 concluídos, quatro reversões, zero falhas RPC e zero diferenças de pré-checagem. Os 50/110 PCs estáticos observados e 58 resultados de desvio permaneceram iguais à rodada anterior. O filtro ficou em `NO_ELIGIBLE_CANDIDATE`; o plano de IA continha zero pedidos. As contagens são desta entrada e deste estado fixado.

A etapa seguinte usa o [plano local](SIMPLE-PLAN.md) como entrada opcional. O chip preserva a própria memória e o plano acrescenta valores diretos, pares de estado e um motivo para encerrar campanhas sem entrada distinta.
