# M4 — candidatos econômicos determinísticos

O M4 acrescenta uma camada de triagem **source-backed** sobre a IR do M2. Ela emite candidatos `UNVERIFIED` com regra, evidências por source span, premissas e confiança `Tentative`. Nenhuma regra mede lucro, prova factibilidade de caminho ou confirma vulnerabilidade. Bytecode isolado do M3 ainda não fornece identidade suficiente de assets, shares, fees e oracles para aplicar essas regras sem source correspondente.

```sh
security-engine --economic-candidates Vault.sol /usr/local/bin/solc > candidates.json
security-engine --economic-candidates Vault.sol /usr/local/bin/solc slither.json > candidates.json
```

O terceiro argumento é um JSON de Slither produzido localmente com `--json`. O wrapper abaixo executa Slither e a análise Rust, salva cópia do source, versões das ferramentas, JSON bruto e candidatos em um diretório novo:

```sh
TOUPERA_SECURITY_ENGINE=/srv/toupera-agent/target/release/security-engine \
  scripts/run-economic-candidates.sh Vault.sol /tmp/vault-m4
```

O parser exige `success=true` e `results.detectors` no JSON do Slither. Importa todos os sinais para `slither_signals`, mas só transforma em candidato os detectores com mapeamento econômico explícito e função não ambígua com span no arquivo analisado: `divide-before-multiply` → arredondamento; `unchecked-transfer` e `arbitrary-send-erc20` → accounting. Outros detectores permanecem visíveis e não são promovidos a hipótese econômica por analogia. Resultados da mesma categoria e função são deduplicados, conservando todas as regras e localizações. Sinais sem mapeamento continuam sem candidato.

## Regras Rust

| Regra | Sinal exigido | Categoria |
| --- | --- | --- |
| `asset-share-conservation-gap` | Transferência de asset observada sem escrita de shares/supply e sem chamada de mint/burn reconhecida | AccountingDrift |
| `entry-exit-accounting-asymmetry` | Entrada e saída escrevem conjuntos diferentes de campos econômicos modelados | AccountingDrift |
| `oracle-preview-value-asymmetry` | Fluxo de oracle chega à conversão/accounting em apenas um lado do par preview/execução | PreviewExecutionMismatch |
| `opposite-price-source-asymmetry` | Entrada e saída usam oracle de maneira desigual no fluxo de valor | PriceSourceAsymmetry |
| `oracle-preview-source-asymmetry` | Preview e execução referenciam alvos de oracle diferentes | OracleFallbackMismatch |
| `oracle-value-without-modeled-guard` | Valor de oracle alcança conversão/accounting sem guarda modelada desse valor | OracleFallbackMismatch |
| `rounding-preview-asymmetry` | Direções explícitas de arredondamento diferem entre preview e execução | RoundingAsymmetry |
| `rounding-preview-formula-difference` | O mesmo numerador e denominador aparecem como `x / d` e `(x + d - 1) / d` no par preview/execução | RoundingAsymmetry |
| `fee-preview-asymmetry` | Referência observada a fee existe em apenas um lado de preview/execução | FeeBypass |
| `conversion-scale-asymmetry` | Conversões recíprocas usam expoentes literais decimais diferentes (`1eN` ou `10**N`) | DecimalMismatch |
| `first-share-donation-sensitivity` | Conversão shares usa supply/assets com ramo zero e `totalAssets` lê `balanceOf`, sem offset virtual literal reconhecido | ShareInflation |

Os nomes de storage, chamadas e aritmética são indícios sintáticos. Um depósito pode atualizar shares em contrato herdado; uma validação de oracle pode ocorrer no próprio feed; escalas diferentes podem ser corretas para tokens com decimals diferentes. Cada candidato registra essas premissas. Pares de operações ambíguos por overload são omitidos e registrados em `unresolved`, junto das incertezas da reconstrução source. A ausência de candidato não é um atestado de segurança, especialmente com imports, caminhos dinâmicos ou chamadas não resolvidas.

O extrator source agora também preserva chamadas explícitas `mulDiv`, `mulDivUp`, `mulDivDown` e `ceilDiv` como expressões aritméticas com span. Isso permite observar o argumento de arredondamento quando ele aparece no próprio call site; não interpreta helpers arbitrários nem calcula a precisão real da operação.

A regra de fórmula reconhece apenas a forma literal com denominador identificador e numerador idêntico nos dois lados. Ela conserva os dois source spans e mantém `UNVERIFIED` mesmo quando o preview contém uma alternativa com arredondamento compatível: é preciso verificar a condição do ramo e comparar valores em estado local. Expressões equivalentes escritas de outra forma podem ficar fora dessa triagem.

## Métricas e limite de validação

`metrics` registra funções, sinais de regras, sinais Slither importados/mapeados, candidatos após deduplicação e sinais fundidos. `false_positives_reviewed` permanece `null` até revisão de fixtures e resultados locais. A calibração de falsos positivos, cobertura e valor incremental do Slither foi **adiada a pedido do usuário** para depois da implementação completa. O relatório não atribui redução de uso de IA nem ganho de detecção sem benchmark. O M5 monta slices locais mínimos dos candidatos deduplicados; o M6 deverá enviar à IA apenas dúvidas que persistirem após as regras e evidências locais. Consulte [Pacote de Contexto](CONTEXT-PACKAGE.md).
