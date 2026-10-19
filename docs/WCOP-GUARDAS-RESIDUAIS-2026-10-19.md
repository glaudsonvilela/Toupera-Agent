# Guardas residuais do runtime wCOP — 2026-10-19

## Recorte

A análise continua no trabalho privado `92273047cbf7439c`, proxy wCOP e bloco Ethereum `26228846`. A etapa dirigida anterior observou 155 de 169 PCs `JUMPI` estáticos, sendo 54 nos dois sentidos e 101 em apenas um. Este documento classifica os 101 e registra 14 chamadas adicionais no mesmo fork local fixado. Nenhuma transação foi enviada à rede principal.

`tools/wcop_residual_guards.py` confere bloco e bytecode, usa `debug_traceCall` e verifica o resultado esperado de cada chamada. Para UUPS, duas implementações simuladas são inseridas **somente no fork Anvil**: uma retorna slot incorreto e outra reverte. Os casos de endereço zero também usam remetente sintético local quando necessário; isso mede a execução do guarda, não a capacidade real de uma conta zero assinar uma transação. O script guarda os sentidos dos saltos, sem persistir a pilha ou o calldata.

## Resultado reproduzido

| Medida | Antes | Depois |
| --- | ---: | ---: |
| Casos adicionais desta etapa | 0 | 14 |
| PCs `JUMPI` observados | 155 | 155 |
| PCs observados nos dois sentidos | 54 | 66 |
| PCs observados em apenas um sentido | 101 | 89 |
| Pares PC/sentido observados | 209 | 221 |
| PCs `JUMPI` ainda não observados | 14 | 14 |
| Próximo PC ausente ou incompatível | 0 | 0 |

Doze sentidos adicionais foram observados. Os guardas de destinatário, remetente, mint, burn e approve com endereço zero, papel já concedido ou ausente, slot UUPS incompatível e `unpause` quando já despausado tiveram a resposta esperada. O delegatecall de upgrade para `approve` concluiu no fork; esse caso não acrescentou sentido novo. Os dois mocks de UUPS são casos de rejeição de entrada inválida, não upgrades reais. Não houve divergência local reproduzível nem vulnerabilidade confirmada.

## Separação dos 89 desvios em um sentido

`tools/wcop_residual_classify.py` recompõe os resultados por caso dos quatro relatórios, confirma a identidade do runtime e classifica cada PC com o padrão de instruções imediatamente anterior e o mapa do compilador.

| Categoria estrutural | PCs | Interpretação para a triagem |
| --- | ---: | --- |
| Dispatcher | 9 | Seleção da função ou comprimento mínimo de calldata antes da função. |
| Entrada sem ETH | 31 | Padrão `CALLVALUE`, `DUP`, `ISZERO`, `PUSH`, `JUMPI` gerado para funções não pagáveis. |
| Utilitário ABI gerado | 25 | Verificações de comprimento, limites ou codificação em `#utility.yul`. |
| Guarda de execução | 24 | Condições que dependem da entrada, estado ou resultado de chamada. |

Os 24 guardas de execução se distribuem em leitura de strings/EIP-712 (6 PCs), `Initializable` (4), UUPS (3), ECDSA (7), verificação de código em ERC-1967 (1) e tratamento de retorno em `Address` (3). Essa separação **não prova** que os lados restantes sejam alcançáveis ou inalcançáveis. Os 31 guardas sem ETH continuam sendo condições do runtime; estão fora da prioridade de lógica econômica nesta triagem. Os 25 PCs Yul precisam de entradas ABI específicas antes de qualquer afirmação sobre risco.

Os 14 PCs totalmente não observados continuam ligados a alternativas de inicialização e armazenamento de strings e a código gerado. A medição do proxy já inicializado não substitui um teste de inicialização em ambiente local separado. O vínculo semântico entre `JUMPI` e IDs LCOV da fonte recompilada continua pendente; contagens de PCs não são cobertura LCOV.

## Evidência privada e próximos testes

- `private-output/site-jobs/92273047cbf7439c/residual-guards.json`: 14 casos, SHA-256 `97aeeaf34acd3c32f1f4f931f570668dc0b8f5e8f9815c63d8944386b745e941`.
- `private-output/site-jobs/92273047cbf7439c/residual-classification.json`: 89 PCs classificados por categoria e local, SHA-256 `870b834941cecf46de2dbd06fb50489bfa893c979e68353a230bee4aafbe9191`.
- Ambos os relatórios têm modo `0600` e pertencem a `toupera`. Os scripts passaram em `py_compile` e foram executados como `toupera` no VPS. O serviço `toupera-site-jobs.service` continuou ativo.

A próxima etapa deve tratar os 24 guardas de execução por pré-condição, começando pelos caminhos de ECDSA e UUPS que ainda têm significado para o proxy fixado. Testes de nome/versão vazios, strings longas e inicialização devem usar estado local preparado e ser contabilizados separadamente. Registrar para cada hipótese a condição necessária, o resultado observado e se existe uma consequência de segurança reproduzível.
