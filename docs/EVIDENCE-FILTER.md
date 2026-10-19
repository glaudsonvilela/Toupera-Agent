# Triagem local de evidências antes da IA

O comando `--evidence-filter` cruza cinco artefatos privados: mapa forense, lente de variáveis, resultados dos cenários no fork, cobertura desses cenários e pacote M5. Ele exige hashes e identidade do mesmo contrato, runtime e bloco. Não consulta RPC nem IA. A execução dos cenários continua sendo feita por `--guided-scenarios` no Anvil local do VPS.

```sh
umask 077
security-engine --evidence-filter forensic.json lens.json scenarios.json coverage.json context.json > filter.json
security-engine --llm-plan context.json --evidence-filter filter.json > plan.json
```

`variable_focus` ordena até 24 declarações pelos sinais já calculados pela lente. `one_sided_source_pcs` aponta até 32 desvios com origem de source e menos de dois resultados observados. `next_scenario_families` indica variações de limite, ordem, ator ou estado para uma próxima rodada local. Essas listas são prioridades, não demonstração de viabilidade ou de comportamento incorreto.

Um resultado só entra em `eligible_candidate_keys` se houver uma diferença concreta após pré-checagem, observada em **duas sequências distintas** para a mesma operação, sem falha RPC ou divergência entre pré-checagem e transação, e se existir candidato M5 da função. Uma observação isolada fica no relatório como inconclusiva. A etapa M6 agora exige `--evidence-filter <filter.json>` tanto para planejar como para revisar; o gate confere o hash exato do pacote M5 e restringe as requisições às chaves elegíveis.

O filtro consome a rodada de cenários; o modo guiado agora acrescenta até 16 variações locais próximas às escalas de asset/shares e para após oito casos sem novo resultado de desvio. Isso ainda não prova que duas sequências sejam independentes em todos os aspectos. A seleção atual de cenários guiados é limitada a famílias conhecidas do vault e pode atingir um platô. Se não houver uma diferença repetida, o estado é `NO_ELIGIBLE_CANDIDATE`, com zero pedidos externos. A próxima melhoria é gerar novas ordens e estados a partir das lacunas específicas de cada desvio. A variação atual é limitada a valores próximos a escalas conhecidas e não é busca exaustiva.
