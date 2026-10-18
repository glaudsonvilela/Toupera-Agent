# M8 — pacote de evidência e entrega

O M8 separa a preparação do pacote da liberação de um relatório. A primeira operação funciona mesmo sem teste executado e conserva o achado pendente:

```sh
security-engine --evidence-plan /caminho/privado/cases/CANDIDATE_KEY /caminho/privado/evidence/CANDIDATE_KEY
```

O diretório de saída precisa ser novo. O comando copia os artefatos do caso Foundry, calcula SHA-256 por arquivo e cria `manifest.json` e `review.json`. Se não houver resultado M7, o estágio é `PENDING_REPRODUCTION`. Se houver duas execuções positivas e consistentes, o estágio é `PENDING_HUMAN_REVIEW`. Um resultado `INCONCLUSIVE` continua pendente. O pacote preserva source, harness, configuração, JSON e traces locais; não chama IA nem envia dados para fora.

`manifest.json` distingue fatos observados, premissas da hipótese e limitações. Os fatos automáticos abrangem somente identidade do source e estado da reprodução. Os campos em `review.json` são **atestados humanos**, nunca preenchidos a partir do modelo. Antes de finalizar, o revisor deve registrar autorização e referência do escopo, versão do alvo, ambiente, versões de Forge/solc, passos de reprodução, resumo, impacto, pré-condições, inferências, limitações adicionais e revisão de duplicidade. Deve também conferir que as asserções demonstram o comportamento alegado e que os arquivos copiados correspondem aos resultados executados.

Depois da revisão e apenas com reprodução positiva:

```sh
security-engine --evidence-finalize /caminho/privado/evidence/CANDIDATE_KEY
```

O gate confere hashes dos artefatos, identidade do caso, status dos dois testes diretamente nos JSON Foundry, hash do harness e os campos obrigatórios de revisão. Então grava `report.md` e `ready.json` com hashes do manifesto, revisão e relatório, e status `REPORT_READY`. Esses arquivos não são recriados se já existirem. A saída é um **relatório privado pronto para revisão/entrega**, sem envio automático a programa, GitHub ou terceiros.

Uma marcação humana em JSON não prova autorização, impacto, ausência de duplicidade ou qualidade semântica do teste. É responsabilidade do revisor conferir o programa e os artefatos. O relatório separa fatos, inferências, premissas, limitações, reprodução e impacto. Não divulgue um pacote com source privado fora do escopo autorizado.

Nesta implementação nenhum alvo, harness, teste Foundry, campanha de fuzzing ou relatório real foi executado. A validação geral e as métricas do M4/M7 seguem adiadas a pedido do usuário.
