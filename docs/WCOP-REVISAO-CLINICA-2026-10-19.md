# Revisão clínica de UUPS e permit no wCOP — 2026-10-19

## Identidade e pergunta

O trabalho `92273047cbf7439c` fixa o proxy wCOP no bloco Ethereum `26228846` e confere o bytecode da implementação antes de cada ensaio. Esta revisão pergunta se os guardas ainda vistos em um sentido apontam para uma condição de segurança na implantação fixada. As sete chamadas clínicas foram feitas em um fork Anvil local; nenhuma transação foi enviada à rede principal. Os rastros da implementação chamada diretamente (`depth=1`) ficam separados dos rastros do proxy por delegatecall (`depth=2`).

## Resultados medidos

| Contexto e chamada | Resultado no fork | Evidência principal |
| --- | --- | --- |
| Implementação direta: `proxiableUUID()` | Concluiu | Retornou exatamente o slot ERC-1967; PC `4540` seguiu o sentido oposto ao observado via proxy. |
| Implementação direta: `upgradeToAndCall` | Reverteu | Seletor `e07c8dba`, `UUPSUnauthorizedCallContext()`; PCs `4145` e `4235` seguiram os sentidos opostos aos do proxy. |
| Implementação direta: `initialize` | Reverteu | Seletor `f92ee8a9`, `InvalidInitialization()`; o construtor desabilita inicializadores nessa implementação. |
| Proxy: `permit` com `r=s=0` ou `v=29` | Reverteu | Seletor `f645eedf`, `ECDSAInvalidSignature()`. |
| Proxy: `permit` com assinatura que recupera outro signatário | Reverteu | Seletor `4b800e46`, `ERC2612InvalidSigner(address,address)`. |
| Proxy: `permit` com `s` alto | Reverteu | Seletor `d78bce0c`, `ECDSAInvalidSignatureS(bytes32)`. |

O `permit` válido e sua repetição já haviam sido testados em snapshot local na etapa anterior: nonce +1 e allowance 1 na primeira execução, assinatura repetida recusada sem mudança de estado. A matriz acima confirma separadamente os erros de assinatura nula, `v` inválido, signatário errado e `s` alto.

A fonte fixada chama `ECDSA.recover(hash, v, r, s)` em `ERC20PermitUpgradeable.sol:66`. Nessa variante, `tryRecover` retorna apenas `NoError`, `InvalidSignature` ou `InvalidSignatureS`; `InvalidSignatureLength` não é produzido por esse caminho. Portanto, o ramo de `_throwError` para comprimento inválido não constitui, por si, uma lacuna alcançável por `permit(v,r,s)`. Isso é uma inferência da cadeia de chamadas da fonte compilada, não uma alegação de inalcançabilidade global de todo PC mapeado à mesma linha.

## Atomicidade do upgrade no proxy

Os ensaios residuais usam quatro códigos de implementação simulados apenas no fork. Dois testam reprovação na consulta UUPS (slot incorreto e reversão); dois retornam o slot correto e depois revertem no delegatecall de configuração, com e sem dados. O retorno com dados foi propagado. Uma transação local com reversão sem dados teve `receipt.status=0` e deixou o slot de implementação ERC-1967 inalterado; o snapshot foi restaurado. Esses testes cobrem a falha de configuração após a checagem UUPS no estado local medido.

O caso com dados acrescentou o sentido oposto do PC `7015` em `Address.sol`. A contagem **do proxy** passou a 155/169 PCs `JUMPI` observados: 67 em ambos os sentidos, 88 em um e 14 não observados, com 222 pares PC/sentido. Os sentidos adicionais vistos na implementação direta não entraram nesses totais. Entre os 88 de um sentido, 23 são guardas de execução; os outros 65 separam-se em 9 do dispatcher, 31 de entrada não pagável e 25 utilitários ABI gerados.

## Limites e próxima hipótese

Os testes não confirmaram vulnerabilidade. A checagem direta de inicialização e os três sentidos UUPS dependem do contexto de chamada; não devem ser somados aos resultados do proxy. Os 23 guardas de execução ainda incluem seis PCs de strings/EIP-712, quatro de inicialização, três de UUPS, sete de ECDSA, um de código ERC-1967 e dois de retorno em `Address`. Essa lista não é contagem de ramos LCOV nem prova de alcançabilidade de lados alternativos. Nome/versão vazios e inicialização fresca exigem estado local preparado, distinto do proxy já implantado.

A próxima verificação útil é alinhar cada um desses guardas à pré-condição do estado fixado e, para qualquer caminho que pareça alcançável, demonstrar uma consequência de segurança em teste isolado e repetível. Não há base, nesta evidência, para um relato de bug à Ripio.

## Evidência privada

- `private-output/site-jobs/92273047cbf7439c/clinical-guards.json`: sete chamadas, SHA-256 `c0ee021c99f00061571a5f3b18ba45855201387c1eab0f39510a3bd05055796b`.
- `private-output/site-jobs/92273047cbf7439c/residual-guards.json`: 16 chamadas residuais, SHA-256 `efa0abf6fe8b5bfada5a18b2348f4c55cda67c9b4a885ed6f551493913647eb7`.
- `private-output/site-jobs/92273047cbf7439c/residual-classification.json`: 88 PCs classificados, SHA-256 `61aedc14006b24f13acfa08474ca8ae9fdc2ca5e080b1017260eb0b45e95e148`.
- Os três JSONs têm modo `0600`, propriedade de `toupera`; os scripts passaram em `py_compile` e o serviço `toupera-site-jobs.service` continuou ativo.
