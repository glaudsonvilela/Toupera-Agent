# M7 — casos locais e juiz Foundry

O M7 recebe o pacote M5 e o relatório M6, confere que o SHA-256 do arquivo Solidity é o mesmo do pacote, e cria um diretório privado por hipótese `UNVERIFIED`. O texto gerado pelo modelo fica em `case.json`; nunca é inserido como código Solidity.

```sh
security-engine --foundry-plan context.json review.json Target.sol /caminho/privado/cases
```

Cada caso contém `src/Target.sol`, `test/Scenario.t.sol`, `foundry.toml`, `case.json` e `harness.json`. O teste é inicialmente um **scaffold que falha**. A pessoa que prepara o harness deve implementar `testReproduce()` e `testFuzzBoundaries(uint256)` para o comportamento alegado, usar entradas que de fato alcançam o caminho econômico, e conferir limites, fees e arredondamentos pertinentes. `harness.json` só deve registrar `reviewed=true`, nota de revisão com pelo menos 20 caracteres, `valid_calls_observed=true` e `boundaries_covered=true` após conferir essas condições. Esses campos são atestados humanos; o motor não consegue provar sua veracidade por inspeção textual.

Depois de implementado e revisado, o comando abaixo executa **somente no projeto Foundry local**. Ele não é acionado por `--foundry-plan` nem por nenhuma etapa automática de IA:

```sh
security-engine --foundry-run /caminho/privado/cases/CANDIDATE_KEY /caminho/absoluto/forge /caminho/absoluto/solc
```

O executor usa `forge test --offline`, com `--use` para o `solc` local, `--match-path test/Scenario.t.sol`, seed fixa, 256 iterações de fuzz e limite de 180 segundos por execução. Ele não passa RPC URL nem habilita FFI; rejeita indicadores óbvios de fork/RPC/FFI no harness. `--offline` e essa inspeção não são isolamento de rede completo: antes de rodar um harness de terceiros, revise o projeto e execute-o em ambiente isolado/autorizado. Não use alvo de produção.

O juiz roda `forge test` duas vezes e guarda JSON e saída `-vvvv` em `runs/`, junto com hashes do source e harness. Para um resultado positivo, exige que ambas as execuções terminem com `Success` em todos os testes encontrados e que `testFuzzBoundaries(uint256)` registre pelo menos 256 entradas executadas por rodada no campo `kind.Fuzz.runs` do JSON Foundry. Depois roda `forge coverage --report lcov` com a mesma seed e guarda `coverage.info` e `coverage.trace.txt`. Exige execução bem-sucedida e ao menos uma linha atingida em `src/Target.sol`; registra linhas e branches cobertos. Falha de compilação, teste, parse ou cobertura fica `INCONCLUSIVE`, sem rejeição automática.

Se o harness contiver testes `invariant*`, o resultado registra os campos Foundry `kind.Invariant.calls` e `kind.Invariant.reverts` por rodada. Esses números são chamadas aos handlers da campanha, **não** uma contagem verificada de chamadas econômicas válidas ao contrato alvo. Sem campanha de invariantes, esses campos ficam ausentes. A revisão humana ainda precisa conferir alcance real, asserções, entradas válidas e reverts esperados. O juiz só emite `REPRODUCED_PENDING_REVIEW`; não emite `CONFIRMED` nem `REPORT_READY`.

Os artefatos devem ficar fora do repositório ou em diretório ignorado, com acesso restrito. O gerador é destinado ao recorte M5 source-first de um arquivo, sem imports. O fluxo não cria automaticamente um reproducer correto a partir de linguagem natural. A instrumentação de LCOV/fuzz/invariantes está implementada, mas nenhuma medição real foi feita; campanha Echidna/Medusa e piloto Halmos dependem de casos executáveis e seguem pendentes até a rodada de validação geral autorizada pelo usuário.

Formato dos resultados: [código de resultado do Foundry 1.8.5](https://github.com/foundry-rs/foundry/blob/v1.8.5/crates/forge/src/result.rs). Saída de cobertura: [documentação oficial do Forge](https://getfoundry.sh/forge/coverage).
