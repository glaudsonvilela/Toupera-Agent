# M8 — pacote de evidência e entrega

O M8 separa a preparação do pacote da liberação de um relatório. A primeira operação funciona mesmo sem teste executado e conserva o achado pendente:

```sh
security-engine --evidence-plan /caminho/privado/cases/CANDIDATE_KEY /caminho/privado/evidence/CANDIDATE_KEY
```

O diretório de saída precisa ser novo. O comando copia os artefatos do caso Foundry, calcula SHA-256 por arquivo e cria `manifest.json` e `review.json`. Se não houver resultado M7, o estágio é `PENDING_REPRODUCTION`. Se houver duas execuções positivas e consistentes, o estágio é `PENDING_HUMAN_REVIEW`. Um resultado `INCONCLUSIVE` continua pendente, salvo se existir `runs/rejection.json` consistente com source, harness, resultado e revisão; nesse caso o estágio é `REJECTED`. Este estágio nunca libera `REPORT_READY`. O pacote preserva source, harness, configuração, JSON e traces locais; não chama IA nem envia dados para fora.

`manifest.json` distingue fatos observados, premissas da hipótese e limitações. Os fatos automáticos abrangem somente identidade do source e estado da reprodução. Os campos em `review.json` são **atestados humanos**, nunca preenchidos a partir do modelo. Antes de finalizar, o revisor deve registrar autorização e referência do escopo, versão do alvo, ambiente, versões de Forge/solc, passos de reprodução, resumo, impacto, pré-condições, inferências, limitações adicionais e revisão de duplicidade. Deve também conferir que as asserções demonstram o comportamento alegado e que os arquivos copiados correspondem aos resultados executados.

Depois da revisão e apenas com reprodução positiva:

```sh
security-engine --evidence-finalize /caminho/privado/evidence/CANDIDATE_KEY
```

O gate confere hashes dos artefatos, identidade do caso, status e contagem de fuzz dos dois testes diretamente nos JSON Foundry, hash do harness, LCOV com cobertura não nula do alvo e os campos obrigatórios de revisão. Então grava `report.md` e `ready.json` com hashes do manifesto, revisão e relatório, e status `REPORT_READY`. Esses arquivos não são recriados se já existirem. A saída é um **relatório privado pronto para revisão/entrega**, sem envio automático a programa, GitHub ou terceiros. Chamadas e reverts de invariantes só aparecem quando a campanha existe; não equivalem automaticamente a chamadas válidas ao alvo.

Uma marcação humana em JSON não prova autorização, impacto, ausência de duplicidade ou qualidade semântica do teste. É responsabilidade do revisor conferir o programa e os artefatos. O relatório separa fatos, inferências, premissas, limitações, reprodução e impacto. Não divulgue um pacote com source privado fora do escopo autorizado.

O [piloto M7](M7-PILOT.md) executou somente fixture sintética própria, sem alvo externo ou relatório real. Métricas e revisão de alvos reais continuam por caso autorizado.
