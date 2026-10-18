# Ferramentas de análise para o recorte econômico

Avaliação em 2026-10-18. Esta é uma decisão de arquitetura, não um resultado de benchmark nem uma integração já executada. O M2 atual reconstrói um arquivo Solidity sem imports; Slither, Foundry e os demais executores ainda não estão integrados ao Toupera Agent.

## Decisão

Manter a IR econômica Rust como índice de funções, evidências e incertezas. Acrescentar ferramentas externas como produtores independentes de sinais e contraexemplos. Executar primeiro regras determinísticas e testes locais; enviar à IA somente candidatos deduplicados com contexto curto quando faltar interpretação econômica. Um alerta de scanner, um teste que passou ou um timeout de prova não confirma nem exclui vulnerabilidade.

| Ferramenta | Utilidade no escopo vault/oracle | Adoção | Papel no fluxo |
| --- | --- | --- | --- |
| [Slither](https://github.com/crytic/slither) | Alta para estrutura, herança, chamadas, dependência de dados e detectores customizados; não substitui invariantes econômicas. | Primeira integração | Triagem source e enriquecimento da IR, com resultados deduplicados por localização e categoria. |
| [Mythril](https://github.com/ConsenSysDiligence/mythril) | Complementar para problemas EVM genéricos e caminhos de bytecode; menor precisão para lógica econômica específica. | Piloto seletivo após M3 | Examinar candidatos de bytecode ou caminhos que o source não esclareceu, com limite de tempo. |
| [Securify2](https://github.com/eth-sri/securify2) | Sobreposição com scanners existentes; a documentação limita a entrada a contratos sem imports e o Docker fixa `solc` 0.5.12 por padrão. | Não integrar ao fluxo principal | Comparação experimental somente se fixtures modernas e custo de manutenção justificarem. |
| [Manticore](https://github.com/trailofbits/manticore) | Exploração simbólica potencialmente útil, mas o projeto foi arquivado pelo mantenedor em 2026. | Não adotar | Manter fora da base do produto. |
| [Foundry](https://getfoundry.sh/forge/invariant-testing) | Alta para reprodução local, fuzzing de entradas e sequências de operações. | Primeiro executor de testes | Testes parametrizados e invariantes de vault em unidade, Anvil ou fork autorizado. |
| [Echidna](https://github.com/crytic/echidna) | Alta para sequências de chamadas que falsificam propriedades, corpus e redução de exemplos. | Segundo fuzzer, condicionado a lacuna medida | Rodar propriedades reutilizáveis quando Foundry não atingir estados relevantes. |
| [Medusa](https://github.com/crytic/medusa) | Alta para campanhas paralelas guiadas por cobertura; API Go de alto nível ainda evolui. | Alternativa ao Echidna, não ambos por padrão | Comparar cobertura, achados únicos e tempo na mesma suíte de propriedades. |
| [Halmos](https://github.com/a16z/halmos) | Alta para condições de contorno aritméticas e caminhos curtos expressos como testes Solidity/Foundry. | Primeiro piloto simbólico | Verificar propriedades focadas reutilizando um harness Foundry executável. |
| [Kontrol](https://docs.runtimeverification.com/kontrol) | Alta para provas selecionadas sobre semântica EVM e reutilização de testes Foundry; maior custo de modelagem/execução. | Caso avançado | Provar uma propriedade de alto impacto quando fuzzing e Halmos não bastarem. |
| [Certora](https://docs.certora.com/en/latest/docs/cvl/index.html) | Alta para invariantes e regras CVL específicas de protocolo, com custo de especificação. | Caso avançado e autorizado | Provas de propriedades críticas após definir premissas e política para o serviço remoto. |

As propriedades [ERC-4626 da Crytic](https://github.com/crytic/properties) oferecem um ponto de partida para vaults compatíveis; precisam ser adaptadas ao accounting, fees e estratégias reais do alvo. Não assumir `totalAssets == token.balanceOf(vault)` nem igualdade exata entre preview e execução sem checar arredondamento, taxas, doações e ativos investidos.

## Ordem de implantação

1. **Base determinística (M4/M5):** modelar conservação, relações shares/assets, limites de rounding, simetria entre preview e operação, comportamento de oracle/decimals e accounting; identificar exceções válidas por contrato. Normalizar `tool`, versão, hash do alvo, contrato/função, span ou PC, propriedade, premissas, severidade, confiança e estado da reprodução.
2. **Slither (M4):** executar sobre projetos compiláveis, importar somente resultados relevantes ao escopo e cruzá-los com a IR Rust. Usar a API de detectores para regras de vault que se mostrem reutilizáveis. Resolver imports e versão de `solc` do alvo antes de comparar outputs; o extrator Rust M2 continua limitado a um arquivo.
3. **Foundry (M7):** começar por testes de limites (zero, primeira emissão, valores extremos, decimals diferentes) e sequências como depositar→doar→resgatar, mudar preço→depositar/resgatar, cobrar fee→converter. Medir chamadas válidas, branches e reverts; um invariante que nunca foi exercitado não conta como validação.
4. **Campanhas adicionais (M7):** usar Echidna **ou** Medusa quando a cobertura ou taxa de achados únicos de Foundry justificar. Reutilizar a mesma propriedade e guardar seed, corpus e sequência mínima para reprodução em Foundry. Não executar três fuzzers em todos os alvos por padrão.
5. **Prova focada (M7/M8):** experimentar Halmos primeiro em propriedades curtas de rounding, conservação ou autorização. Avaliar Kontrol ou Certora somente para questões críticas que compensem a especificação e os recursos exigidos. `UNKNOWN`, timeout ou premissas incompletas permanecem sem conclusão.

## Quando a IA entra

```text
AST/bytecode + Rust/Slither → regra econômica → teste/invariante local
                                   ↓ candidato deduplicado ainda ambíguo
                         slice + premissas + evidências → IA
                                   ↓ hipótese verificável
                           Foundry → revisão humana
```

As ferramentas reduzem chamadas à IA **somente se** seus resultados virarem gates: descartar duplicatas, revisar premissas refutadas por evidência determinística, priorizar caminhos relevantes e enviar um único slice por hipótese. Um fuzz test que passa não elimina o candidato por si só. As ferramentas também podem descobrir bugs que um LLM perderia, sobretudo estados obtidos por sequências, entradas raras e divergências aritméticas. A economia de tokens e o ganho de bugs ainda precisam ser medidos; não há percentual comprovado para este repositório.

Registrar por alvo: funções analisadas, candidatos por ferramenta, sobreposição, achados únicos reproduzidos, falsos positivos revisados, cobertura/estado alcançado, minutos de CPU, chamadas e tokens de IA. Comparar a mesma fixture com e sem cada ferramenta. Um bug só muda para `REPRODUCED` após teste local determinístico; `CONFIRMED` exige revisão humana.

## Limites operacionais

- Os resultados de análise estática e simbólica dependem do código, versão do compilador, modelo de ambiente e premissas fornecidas. Propriedade mal especificada pode produzir falsa confiança.
- O [CLI padrão do Certora envia o trabalho aos servidores do fornecedor](https://docs.certora.com/en/latest/docs/user-guide/running.html). Para um alvo privado, não submeter source, bytecode ou especificações a serviço externo sem autorização específica e revisão do que será enviado. Halmos e Kontrol podem ser avaliados localmente primeiro.
- Executar testes somente em contratos próprios, fixtures, Anvil ou forks locais dentro da autorização do alvo. Não transformar contraexemplos em transações ofensivas de produção.

## Fontes primárias

- [Slither e detectores customizados](https://github.com/crytic/slither); [Mythril](https://github.com/ConsenSysDiligence/mythril); [Securify2](https://github.com/eth-sri/securify2); [Manticore arquivado](https://github.com/trailofbits/manticore).
- [Foundry: invariant testing](https://getfoundry.sh/forge/invariant-testing); [Echidna](https://github.com/crytic/echidna); [Medusa](https://github.com/crytic/medusa); [propriedades Crytic ERC-4626](https://github.com/crytic/properties).
- [Halmos](https://github.com/a16z/halmos); [Kontrol](https://docs.runtimeverification.com/kontrol); [Certora CVL](https://docs.certora.com/en/latest/docs/cvl/index.html) e [execução remota](https://docs.certora.com/en/latest/docs/user-guide/running.html).
