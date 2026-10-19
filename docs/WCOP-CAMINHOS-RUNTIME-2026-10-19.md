# wCOP: caminhos observados no runtime publicado

- Trabalho: `154aa11a5aa144ed`; Ethereum, bloco `26227976`.
- Endereço: `0x8a1d45e102e886510e891d2ec656a708991e2d76`.
- Método: chamadas isoladas e uma sequência com estado sobre uma cópia local do bloco; o bytecode vem da implementação publicada e o motor Rust confere sua identidade antes de ligar PCs ao mapa.
- Nenhuma chamada deste ensaio foi enviada como transação à rede pública.

## Medição

| Item | Observado | Total do mapa |
|---|---:|---:|
| PCs de instruções | 2470 | 5377 |
| Blocos | 241 | 559 |
| Ligações entre blocos presentes no mapa | 235 | 296 transições observadas |

- Chamadas: 36; sequência: 12; passos gravados só na cópia local: 10; divergências de resultado esperado: 0; revertidas conforme esperado ou por condição local: 7; erros da leitura: 0.
- Transições observadas ainda sem ligação no mapa estático: 61. Isso aponta trabalho no mapa, não uma falha do contrato.
- Dúvidas do motor em PCs visitados: 202. A lista ordenada fica no JSON privado; a presença no caminho não confirma problema.
- Medição separada da fonte recompilada: 38/213 ramos no conjunto fonte e bibliotecas; veja [a segunda medição](WCOP-COBERTURA-ETAPA2-2026-10-19.md).

## Chamadas locais

| Caso | Grupo | Seletor | Estado local | PCs distintos |
|---|---|---|---|---:|
| DEFAULT_ADMIN_ROLE() | isolada | `0xa217fddf` | concluiu | 63 |
| DOMAIN_SEPARATOR() | isolada | `0x3644e515` | concluiu | 368 |
| MINTER_ROLE() | isolada | `0xd5391393` | concluiu | 62 |
| PAUSER_ROLE() | isolada | `0xe63ab1e9` | concluiu | 87 |
| UPGRADER_ROLE() | isolada | `0xf72c0d8b` | concluiu | 82 |
| UPGRADE_INTERFACE_VERSION() | isolada | `0xad3cb1cc` | concluiu | 148 |
| decimals() | isolada | `0x313ce567` | concluiu | 74 |
| eip712Domain() | isolada | `0x84b0196e` | concluiu | 445 |
| name() | isolada | `0x06fdde03` | concluiu | 251 |
| pause() | isolada | `0x8456cb59` | reverteu | 172 |
| paused() | isolada | `0x5c975abb` | concluiu | 90 |
| proxiableUUID() | isolada | `0x52d1902d` | reverteu | 87 |
| symbol() | isolada | `0x95d89b41` | concluiu | 255 |
| totalSupply() | isolada | `0x18160ddd` | concluiu | 78 |
| unpause() | isolada | `0x3f4ba83a` | reverteu | 167 |
| balanceOf(holder) | isolada | `0x70a08231` | concluiu | 150 |
| transfer(holder) | isolada | `0xa9059cbb` | concluiu | 356 |
| burn(holder) | isolada | `0x42966c68` | concluiu | 276 |
| approve(holder) | isolada | `0x095ea7b3` | concluiu | 271 |
| mint(authorized) | isolada | `0x40c10f19` | concluiu | 389 |
| mint(other) | isolada | `0x40c10f19` | reverteu | 216 |
| pause(authorized) | isolada | `0x8456cb59` | concluiu | 222 |
| pause(other) | isolada | `0x8456cb59` | reverteu | 172 |
| grantRole(admin) | isolada | `0x2f2ff15d` | concluiu | 325 |
| sequência: emitir | sequência | `0x40c10f19` | concluiu | 389 |
| sequência: autorizar | sequência | `0x095ea7b3` | concluiu | 271 |
| sequência: mover com autorização | sequência | `0x23b872dd` | concluiu | 528 |
| sequência: retirar | sequência | `0x42966c68` | concluiu | 276 |
| sequência: pausar | sequência | `0x8456cb59` | concluiu | 222 |
| sequência: movimento pausado | sequência | `0xa9059cbb` | reverteu | 184 |
| sequência: retomar | sequência | `0x3f4ba83a` | concluiu | 212 |
| sequência: movimento retomado | sequência | `0xa9059cbb` | concluiu | 356 |
| sequência: conceder papel | sequência | `0x2f2ff15d` | concluiu | 325 |
| sequência: emitir com papel | sequência | `0x40c10f19` | concluiu | 389 |
| sequência: retirar papel | sequência | `0xd547741f` | concluiu | 298 |
| sequência: emitir após retirada | sequência | `0x40c10f19` | reverteu | 216 |

## Limites e próximos passos

- As chamadas isoladas não guardam estado entre si; a sequência local usa um conjunto limitado de operações e papéis. Outras combinações podem percorrer outros PCs.
- O total de instruções desmontadas não é o total comprovado de instruções alcançáveis. Portanto, a razão observada não é porcentagem de cobertura completa do contrato.
- Próxima etapa: ampliar sequências com estado na cópia local e ligar as transições ainda ausentes ao mapa Rust, com verificação de identidade em cada rodada.
- Nenhuma falha do contrato foi confirmada por esta medição.
