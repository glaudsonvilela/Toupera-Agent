# Desvios dirigidos do runtime wCOP — 2026-10-19

## Recorte e método

O trabalho privado `92273047cbf7439c` fixa o contrato, o runtime e o bloco Ethereum `26228846`. A etapa anterior repetiu 36 casos iniciais, 38 escolhas do Rust e os 65 candidatos restantes em cópias locais do Anvil. Essa amostra havia observado 111 dos 169 PCs `JUMPI` estáticos, sem diferença local confirmada.

`tools/wcop_targeted_guards.py` confere novamente a identidade do bloco e do bytecode, abre um fork local e executa 22 chamadas dirigidas com `debug_traceCall`. Guarda por caso a condição, o destino e o próximo PC de cada `JUMPI`, sem persistir a pilha nem calldata no relatório. As chamadas cobrem inicialização, UUPS, permissões, ERC-165, allowance, nonces e permit. Nenhuma transação foi enviada à rede principal.

## Resultado

| Medida no runtime fixado | Valor |
| --- | ---: |
| PCs `JUMPI` estáticos | 169 |
| PCs `JUMPI` observados após os 22 casos dirigidos | 155 |
| Observados nos dois sentidos | 54 |
| Observados em um sentido | 101 |
| PCs `JUMPI` ainda não observados | 14 |
| Pares distintos PC/sentido observados | 209 |
| Eventos sem próximo PC / próximo PC incompatível | 0 / 0 |

Os 22 casos acrescentaram 44 PCs `JUMPI` e 64 pares PC/sentido ao conjunto anterior. **O total estático de 169 não é um denominador de caminhos alcançáveis**; 101 PCs vistos em um único sentido ainda não têm o lado alternativo justificado ou descartado.

- `initialize` com argumentos válidos reverteu no proxy já inicializado; a chamada com valor também reverteu. Ambas permitiram observar o teste de entrada que antes faltava na fonte própria.
- `upgradeToAndCall` por conta sem papel reverteu; a chamada autorizada para a mesma implementação concluiu no `debug_traceCall`. Implementação inválida, selector desconhecido no delegatecall e valor indevido reverteram; delegatecall para `name()` concluiu. Esses casos só verificam as condições executadas no fork e não autorizam uma conclusão sobre upgrades arbitrários.
- Uma assinatura EIP-2612 gerada por conta de desenvolvimento do Anvil concluiu. Dentro de um snapshot local, a transação aumentou `nonces(owner)` em um e ajustou `allowance` para 1. A mesma assinatura falhou depois, sem alterar esses valores; o snapshot foi revertido. Prazo vencido, `s` alto e `v` inválido também reverteram. Nenhuma chave ou assinatura foi salva no relatório.
- As chamadas ERC-165 para interface AccessControl, ERC-165 e identificador desconhecido, além das leituras externas de allowance e nonce, explicaram outros PCs antes não vistos.

Não houve divergência local reproduzível nesses casos. Reversões esperadas não foram classificadas como falhas do contrato.

## Lacuna residual

Os 14 PCs ainda não observados se dividem em seis com local de fonte e oito de Yul gerado pelo compilador:

- Um está na leitura do nome em `ERC20Upgradeable.sol`. O nome atual tem 15 bytes; essa linha pode conter um caminho alternativo de armazenamento de string, que não foi exercitado.
- Três estão em `Initializable.sol`, após ou dentro de condições de inicialização. O proxy fixado já está inicializado; exercitar um proxy recém-criado seria outra condição de estado e precisaria de contagem separada.
- Dois estão nas alternativas de nome/versão vazios em `EIP712Upgradeable.sol`; a assinatura válida confirma o domínio usado na condição atual, não essas alternativas.
- O Yul gerado aponta para `clean_up_bytearray_end_slots_string_storage` e `copy_byte_array_to_storage_from_t_string_memory_ptr_to_t_string_storage`, incluindo escrita de strings longas. Essas rotinas são usadas na preparação de armazenamento de strings e não apareceram na amostra do proxy já inicializado.

Essa classificação vem da fonte e do código gerado guardados no trabalho. Ela **não prova** que os 14 PCs sejam globalmente inalcançáveis. O próximo recorte útil é examinar os 101 PCs vistos em apenas um sentido por função, pré-condição e risco, sem tratar comparações do dispatcher como condições econômicas. O vínculo exato com IDs de ramo LCOV continua pendente; o LCOV mede a fonte recompilada. O estado histórico antigo também permanece indisponível no RPC (403).

## Evidência e validação

O relatório por caso está em `private-output/site-jobs/92273047cbf7439c/targeted-guards.json`, modo `0600`, propriedade de `toupera`, SHA-256 `47695f05e78e0e9317fa16450e446be26496b8a8fb23342bbe1c9403890b9611`. O script tem SHA-256 `d394718a83d14b94aa5360ac3871d195602d5989e5db4e725aa546beecde4daa`. `py_compile` passou no VPS, o script concluiu como `toupera`, e o serviço `toupera-site-jobs.service` permaneceu ativo.
