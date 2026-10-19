# Triagem local de evidências antes da IA

O comando `--evidence-filter` cruza cinco artefatos privados: mapa forense, lente de variáveis, resultados dos cenários no fork, cobertura desses cenários e pacote M5. Ele exige hashes e identidade do mesmo contrato e bloco. M4/lente/cenários usam hash do runtime exato; M5 registra runtime normalizado e fica ligado ao M4 pelo hash do relatório forense. Não consulta RPC nem IA. A execução dos cenários continua sendo feita por `--guided-scenarios` no Anvil local do VPS.

A seleção de valores desse modo pode usar o [chip adaptativo](ADAPTIVE-CHIP.md). Suas classes e prioridades orientam somente a coleta local; não substituem as exigências de identidade e diferença repetida deste filtro.

```sh
umask 077
security-engine --evidence-filter forensic.json lens.json scenarios.json coverage.json context.json > filter.json
security-engine --llm-plan context.json --evidence-filter filter.json > plan.json
```

`variable_focus` percorre até quatro arestas de dependência para trás a partir de usos em funções econômicas e ordena até 24 declarações pela prioridade da lente e distância desses usos. `one_sided_source_pcs` aponta até 32 desvios com origem de source e menos de dois resultados observados. `next_scenario_families` indica variações de limite, ordem, ator ou estado para uma próxima rodada local. Essas listas são prioridades, não demonstração de viabilidade ou de comportamento incorreto.

Um resultado só entra em `eligible_candidate_keys` se houver uma diferença concreta após pré-checagem, observada em **duas sequências distintas** para a mesma operação, sem falha RPC ou divergência entre pré-checagem e transação, e se existir candidato M5 da função. Uma observação isolada fica no relatório como inconclusiva. A etapa M6 agora exige `--evidence-filter <filter.json>` tanto para planejar como para revisar; o gate confere o hash exato do pacote M5 e restringe as requisições às chaves elegíveis.

O filtro consome a rodada de cenários; o modo guiado agora acrescenta até 16 variações locais próximas às escalas de asset/shares e para após oito casos sem novo resultado de desvio. Isso ainda não prova que duas sequências sejam independentes em todos os aspectos. A seleção atual de cenários guiados é limitada a famílias conhecidas do vault e pode atingir um platô. Se não houver uma diferença repetida, o estado é `NO_ELIGIBLE_CANDIDATE`, com zero pedidos externos. A próxima melhoria é gerar novas ordens e estados a partir das lacunas específicas de cada desvio. A variação atual é limitada a valores próximos a escalas conhecidas e não é busca exaustiva.

## Leitura no VPS (2026-10-19)

O serviço local concluiu 44 casos: 40 operações concluídas, quatro reversões esperadas. A rodada de feedback fez oito variações e parou após oito sem novo resultado de desvio. Não houve diferença de estado registrada, falha RPC ou pedido externo elegível. Esses números descrevem apenas o alvo e as entradas observadas.

## Segunda rodada de precisão

A lente agora extrai comparações diretas entre parâmetro numérico e literal na AST do source de análise, com função, identificador, valor, operador e span. O modo guiado usa valores imediatamente abaixo, no ponto e acima quando o orçamento local permite. Quando não há tal comparação na função econômica, ele procura uma mudança de `previewMint` ou `previewRedeem` numa janela limitada, usando consultas somente ao Anvil local; confere os valores adjacentes antes de gerar três entradas. Essas consultas servem para escolher entradas, não para inferir monotonicidade geral.

Depois, o modo guiado executa até oito sequências de ordem/estado, cada uma isolada por snapshot. Há duas paradas: no máximo 16 entradas adicionais na rodada de valores, e parada por platô de oito casos sem novo resultado no ponto alvo; na rodada de sequência, no máximo oito sequências e parada após quatro sem resultado novo. A saída registra contadores e motivos. Todos os envios de transação continuam restritos ao Anvil em `127.0.0.1`.

O filtro também inclui `function_reach` com PCs estáticos atribuídos a uma função, PCs de fato observados e PCs vistos nos dois sentidos. Um PC não observado recebe `unknown_reachability`: não se presume que seja alcançável. `mapping_relation_checks` cruza fórmula de slot observada por endereço com o layout do source e marca coincidência, sem concluir igualdade geral de chaves. `call_observations` registra alvo de chamada visto no frame raiz, presença de execução filha e mudança de saldo no mesmo caso; a coexistência não atribui causalidade nem o trecho exato do source. Relações sem essa conferência permanecem pendentes.


Rodada `precision-v2` no VPS (2026-10-19): serviço `toupera-precision-transition-20261019.service` terminou com status 0. A lente listou 12 comparações diretas em funções auxiliares, nenhuma aplicável diretamente às quatro operações principais. A escolha por mudança de preview fez 68 consultas locais e originou seis casos; a rodada de valores parou após oito casos sem resultado novo no ponto alvo. Quatro sequências de ordem/estado foram executadas antes do próprio platô. Total: 50 casos, 46 concluídos, quatro reversões esperadas, zero falhas RPC e zero diferenças de pré-checagem. O conjunto observado permaneceu em 50/110 PCs estáticos, sem resultado novo; 41 PCs têm origem única de source, 23 deles observados. O filtro registrou 25 resumos por função, duas coincidências de slot/layout, quatro acessos indexados sem coincidência, um alvo de chamada observado e zero pedidos de IA. Todas as contagens se limitam ao estado e às entradas da rodada.
