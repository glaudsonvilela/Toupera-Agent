# M6 — hipóteses econômicas com IA

O M6 recebe o JSON local do M5 e prepara **uma pergunta por candidato econômico deduplicado**. A interface Rust `HypothesisProvider` separa a seleção e validação da hipótese do transporte. O primeiro adaptador é DeepSeek Chat Completions. Nenhum uso da API ocorre na compilação ou no comando de planejamento.

## Planejamento sem rede

```sh
umask 077
mkdir -m 700 -p private-output
./target/release/security-engine --llm-plan private-output/context.json > private-output/llm-plan.json
```

O plano aceita o pacote M5 bruto ou o envelope `{cache_hit, cache_key, package}` do CLI. Só entram candidatos `UNVERIFIED` com evidência, perguntas de invariantes e trecho de função. Duplicatas com mesmo contrato, função, categoria e hash de source da função são removidas. O corpo enviado é reconstruído a partir de uma lista explícita de campos do slice: função, até três funções relacionadas, evidências, storage, aritmética, alvos de oracle, chamadas, perguntas e incertezas. O pacote M5 inteiro, outros arquivos e JSON bruto do Slither não são enviados. A saída do plano ainda contém trechos privados; proteja o arquivo.

Se a triagem M4 produzir zero candidatos, `--llm-review <context.json>` devolve um relatório vazio local (`provider=none`, zero chamadas/tokens) sem exigir provedor, chave ou permissão de envio. M7 recebe zero casos e M8 permanece sem entrada. Isso foi observado no [piloto Sepolia](REAL-EVM-PILOT.md) e não implica que o alvo esteja seguro.

## Chamada externa opcional

O comando abaixo **envia esses trechos privados à DeepSeek**. Use-o somente para um alvo cujo escopo permita esse processamento externo. A chave é lida de `TOUPERA_DEEPSEEK_API_KEY` no ambiente e não aparece em argumento de linha de comando nem em arquivo do projeto.

```sh
umask 077
./target/release/security-engine --llm-review private-output/context.json \
  --provider deepseek --allow-external \
  --review-cache private-output/llm-cache \
  --model deepseek-flash --max-candidates 5 > private-output/llm-review.json
```

`--allow-external`, `--provider deepseek` e `--review-cache` são obrigatórios. O limite padrão é cinco candidatos por execução, configurável de 1 a 20. O cache de respostas válidas usa hash do prompt, fornecedor e modelo, em diretório `0700` e arquivos `0600`; uma resposta reaproveitada não faz nova chamada. Não há retry automático. Erro de rede, resposta truncada ou JSON inválido interrompe o lote e fica registrado no relatório. O CLI retorna código de erro nesses casos.

O prompt pede JSON com `disposition` igual a `hypothesis` ou `insufficient_evidence`. Uma hipótese exige `hypothesis`, `assumptions`, `relevant_paths` e `suggested_test`; uma resposta insuficiente não pode preencher esses campos. O parser valida a estrutura, mas **não verifica a verdade do conteúdo**. O status de cada item permanece `UNVERIFIED`. O código do contrato e seus comentários são tratados como dados, não como instruções ao modelo. O M7 terá de criar e executar a reprodução local antes de qualquer avanço de estado.

## Uso e custo

O relatório separa candidatos recebidos, elegíveis, ignorados e selecionados; chamadas externas concluídas com resposta válida da API; respostas reaproveitadas; erros; e tokens de entrada, saída e cache informados pela API. Uma falha de transporte pode ter chegado ao servidor sem resposta: seu consumo não pode ser medido pelo CLI. Os totais de tokens e custo da execução contam apenas respostas externas novas recebidas. Para estimar custo, forneça `--rate-card rates.json` com os campos `prompt_cache_hit_usd_per_million`, `prompt_cache_miss_usd_per_million` e `completion_usd_per_million`, preenchidos com as tarifas vigentes do modelo e horário. Sem rate card ou sem quebra de cache na resposta, `estimated_cost_usd` fica `null`. Preços não são fixados no código porque variam. Consulte a [documentação oficial de preços](https://api-docs.deepseek.com/quick_start/pricing/) e a [API de Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/).

Nenhuma chave DeepSeek foi instalada no repositório ou VPS nesta etapa. Nenhuma chamada à API, fixture, teste automatizado ou benchmark foi executado; a validação foi adiada a pedido do usuário até a implementação geral estar completa.
