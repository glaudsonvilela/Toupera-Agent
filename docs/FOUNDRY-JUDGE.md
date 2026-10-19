# M7 — casos locais e juiz Foundry

O M7 recebe o pacote M5 e o relatório M6, confere que o SHA-256 do arquivo Solidity é o mesmo do pacote, e cria um diretório privado por hipótese `UNVERIFIED`. O texto gerado pelo modelo fica em `case.json`; nunca é inserido como código Solidity.

```sh
security-engine --foundry-plan context.json review.json Target.sol /caminho/privado/cases
```

Cada caso contém `src/Target.sol`, `test/Scenario.t.sol`, `foundry.toml`, `case.json`, `harness.json` e `rejection-review.json`. O teste é inicialmente um **scaffold que falha**. A pessoa que prepara o harness deve implementar `testReproduce()` e `testFuzzBoundaries(uint256)` para o comportamento alegado, usar entradas que de fato alcançam o caminho econômico, e conferir limites, fees e arredondamentos pertinentes. `harness.json` só deve registrar `reviewed=true`, nota de revisão com pelo menos 20 caracteres, `valid_calls_observed=true` e `boundaries_covered=true` após conferir essas condições. Esses campos são atestados declarados; o motor não consegue provar sua veracidade por inspeção textual.

Depois de implementado e revisado, o comando abaixo executa **somente no projeto Foundry local**. Ele não é acionado por `--foundry-plan` nem por nenhuma etapa automática de IA:

```sh
security-engine --foundry-run /caminho/privado/cases/CANDIDATE_KEY /caminho/absoluto/forge /caminho/absoluto/solc
```

O executor usa `forge test --offline`, com `--use` para o `solc` local, `--match-path test/Scenario.t.sol`, seed fixa, 256 iterações de fuzz e limite de 180 segundos por execução. Ele não passa RPC URL nem habilita FFI; rejeita indicadores óbvios de fork/RPC/FFI no harness. `--offline` e essa inspeção não são isolamento de rede completo: antes de rodar um harness de terceiros, revise o projeto e execute-o em ambiente isolado/autorizado. Não use alvo de produção.

O juiz roda `forge test` duas vezes e guarda JSON e saída `-vvvv` em `runs/`, junto com hashes do source e harness. Para um resultado positivo, exige que ambas as execuções terminem com `Success` em todos os testes encontrados e que `testFuzzBoundaries(uint256)` registre pelo menos 256 entradas executadas por rodada no campo `kind.Fuzz.runs` do JSON Foundry. Depois roda `forge coverage --report lcov` **apenas para o teste fuzz**, com a mesma seed, e guarda `coverage.info` e `coverage.trace.txt`. Isso permite medir alcance mesmo quando `testReproduce()` falha. Um resultado positivo exige cobertura bem-sucedida e ao menos uma linha atingida em `src/Target.sol`; registra linhas e branches cobertos. Falha de compilação, teste, parse ou cobertura fica `INCONCLUSIVE`, sem rejeição automática.

Para rejeitar uma hipótese que falhou sob premissas verificadas, preencha `rejection-review.json` com identidade do revisor, referência de escopo, motivo específico e confirmação de premissas, entradas válidas, chamadas ao alvo e asserção falha. Depois execute:

```sh
security-engine --foundry-reject /caminho/privado/cases/CANDIDATE_KEY
```

O gate exige duas execuções com `testReproduce()` em `Failure`, fuzz em `Success` com pelo menos 256 entradas por rodada, demais testes em `Success`, LCOV do alvo, traces e hashes consistentes. Grava `runs/rejection.json` uma única vez, com `REJECTED` e referência criptográfica à revisão e ao resultado. Rejeição é limitada às premissas documentadas; falha de compilação, timeout, falta de cobertura ou fuzz insuficiente jamais satisfazem o gate. Os campos de revisão são atestados declarados e precisam ser conferidos de fato.

Se o harness contiver testes `invariant*`, o resultado registra os campos Foundry `kind.Invariant.calls` e `kind.Invariant.reverts` por rodada. Esses números são chamadas aos handlers da campanha, **não** uma contagem verificada de chamadas econômicas válidas ao contrato alvo. Sem campanha de invariantes, esses campos ficam ausentes. A revisão humana ainda precisa conferir alcance real, asserções, entradas válidas e reverts esperados. O juiz só emite `REPRODUCED_PENDING_REVIEW`; não emite `CONFIRMED` nem `REPORT_READY`.

Os artefatos devem ficar fora do repositório ou em diretório ignorado, com acesso restrito. O gerador é destinado ao recorte M5 source-first de um arquivo, sem imports. O fluxo não cria automaticamente um reproducer correto a partir de linguagem natural. O [piloto M7](M7-PILOT.md) executou uma fixture própria; seu caso foi montado manualmente porque a triagem M4 não produziu candidato para essa fórmula. Isso limita o que o piloto demonstra sobre o fluxo completo.

Formato dos resultados: [código de resultado do Foundry 1.8.5](https://github.com/foundry-rs/foundry/blob/v1.8.5/crates/forge/src/result.rs). Saída de cobertura: [documentação oficial do Forge](https://getfoundry.sh/forge/coverage).
