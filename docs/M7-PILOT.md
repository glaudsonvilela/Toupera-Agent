# M7 — piloto local de reprodução, rejeição e prova curta

Executado em 2026-10-18 no VPS, em diretório privado `private-output/m7-pilot-v2/`, com a fixture própria `tests/fixtures/RoundingVaultM7Fixture.sol`. Não houve rede EVM, fork, envio à DeepSeek, transação em produção nem alvo de terceiros. O source fixa 9 assets, 10 shares e fee de 100 bps. O parâmetro `faultyPreview` alterna somente a direção de arredondamento do preview; `withdraw` sempre arredonda shares para cima.

## Caminhos observados

| Ramo | `previewWithdraw(1)` | `withdraw(1)` | Duas rodadas Foundry | Fuzz por rodada | LCOV do alvo |
| --- | ---: | ---: | --- | ---: | --- |
| `faultyPreview=true` | 2 | 3 | `REPRODUCED_PENDING_REVIEW`; três testes passaram em cada rodada | 256 | 16/18 linhas; 3/5 branches |
| `faultyPreview=false` | 3 | 3 | `testReproduce()` falhou nas duas rodadas; os outros dois passaram | 256 | 17/18 linhas; 2/5 branches |

`testValidDomainAndFeeRounding()` percorre explicitamente os sete valores válidos de assets em cada rodada, confere a fee sintética de uma unidade e as relações de preview/execução. `testFuzzBoundaries(uint256)` mapeia 256 entradas por rodada para esse domínio e chama ambas as funções. Cada harness faz uma chamada válida a `withdraw` por iteração de fuzz, sete no teste determinístico e uma no teste de reprodução: 528 chamadas de `withdraw` por ramo somando as duas rodadas. Essa contagem vem da estrutura do harness e dos status dos testes; o JSON Foundry não fornece um contador independente de chamadas econômicas ao alvo. O único revert esperado no ramo corrigido é a asserção do teste de reprodução, **depois** de chamadas válidas ao alvo.

Após inspeção do source, harness e traces, `--foundry-reject` registrou `REJECTED` apenas para a hipótese sintética no ramo corrigido, com `rejection-review.json` identificado como revisão do piloto pelo Codex, sem se apresentar como auditoria humana. A rejeição não diz nada sobre outros estados iniciais, sequências ou contratos reais.

O piloto Halmos em `tests/fixtures/m7/SymbolicScenario.t.sol` encontrou o contraexemplo `assets=1` para `preview == withdraw` no ramo defeituoso, em oito caminhos e 0,19 s. Uma primeira tentativa que fazia `new` no teste Foundry retornou erro de cheat code `deployCode` não suportado; a propriedade simbólica final usa herança do alvo e não depende desse cheat code. A falha final de Halmos é um contraexemplo esperado, não uma falha de instalação.

## Decisão sobre campanhas adicionais

A cobertura de branches por ramo é parcial porque o outro valor de `faultyPreview` fica fora do escopo daquele caso. Os sete inputs válidos, a fee e o arredondamento já foram exercitados; não há estado sequencial ou branch crítico inacessível que justifique Echidna ou Medusa neste piloto. Para um alvo real, escolher **um** deles se Foundry deixar estados ou branches críticos sem alcance, ou se a mesma propriedade puder produzir sequências/achados únicos; guardar seed, corpus, tempo e contraexemplo e reexecutar o mínimo em Foundry. Halmos é o primeiro piloto para aritmética curta. Kontrol ou Certora só entram se uma propriedade crítica exigir prova mais ampla com premissas explícitas; este caso sintético não justifica esse custo nem envio externo.

## Limites para o fluxo completo

O `--context-package` analisou a fixture, mas o `--llm-plan` retornou **zero** candidatos M4/M5. Por isso, os casos do piloto foram preparados manualmente no diretório privado com o mesmo esquema M7; o piloto valida o juiz, a rejeição e os executores, **não** a seleção M4 nem a preparação M5→M6→M7. A lacuna de detecção de arredondamento nessa fórmula permanece no backlog M4. Os gates `harness.json` e `rejection-review.json` dependem de revisão semântica verdadeira; seus booleanos não a comprovam.

Artefatos brutos, incluindo JSON, traces, LCOV, revisão e contraexemplo Halmos, permanecem no VPS em `/srv/toupera-agent/private-output/m7-pilot-v2/` com acesso restrito. O piloto não gerou relatório de vulnerabilidade real nem `REPORT_READY`.
