# Revisão completa do wCOP — rodada 7011fcd042db41e5

## Escopo e vínculo

- Contrato: `0x8a1d45e102e886510e891d2ec656a708991e2d76` na Ethereum; bloco `26228465`; hash do bloco `0x8cab8eb17fd8cf60691bf40438fd136325cdfa66256e0635924eb852e33ea01c`.
- Implementação no bloco: `0x6f6206fac28d9f60d221a63aae5fb5c3a9ea6652`; código e fonte com vínculo `masked_immutables` e ressalva dos campos imutáveis.
- Entrada `smart.md`, saída Rust `result_smart.md` e resultado integrado `end_smart_result.md` foram lidos deste mesmo trabalho no VPS. Hashes SHA-256:
  - `smart.md`: `c0e1e007ff379ac2bc4132786b6d609756ff0f9935af25a6cfaf66fe008110f1`.
  - `result_smart.md`: `63549471c3e66f8b7b0460a4755d1830a826fa6ff12e75751797817730bcf384`.
  - `end_smart_result.md`: `4b1c95deab05ca8a018a3813e177c97ed5cdea4cc2d802d9bcd687354cfb6c75`.
- Execução por SSH com chave do servidor conferida e cifra `chacha20-poly1305@openssh.com`. As operações que mudam estado foram feitas somente na cópia local.

## Resultado que os arquivos sustentam

- Histórico completo para o intervalo consultado: 236 registros em 5 páginas; emissão menos retirada confere com a oferta lida no bloco.
- Fonte recompilada: 38/213 ramos medidos no conjunto compilado; 38/77 nos arquivos com alguma linha executada. Essa medição não inclui os ramos acrescentados pelo ciclo de bytecode.
- Bytecode: 3048/5377 pontos observados após 40 escolhas; ganho de 578 pontos, 50 ligações e 38 blocos sobre a leitura inicial.
- O ciclo registrou 18 sequências preparadas, 11 passagens para blocos novos e 0 diferenças nas verificações de estado. Parou por `limite`.
- Foundry: 18/18 casos locais; Echidna e Medusa: 2 regras cada em campanhas limitadas; Halmos: 3 propriedades locais limitadas. O motor Rust ligou os caminhos ao runtime com 0 erros de conferência.
- **Falhas confirmadas nesta rodada: 0.** Prioridade alta, média ou baixa abaixo indica ordem de revisão e possível impacto de uma diferença futura, não gravidade de uma falha demonstrada.
- 12 registros de atenção alta por impacto potencial: 4 mint, 5 role_granted, 2 role_revoked, 1 upgraded. A oferta final confere com os eventos; essa fila não demonstra uma diferença no contrato.

## Metas para a próxima rodada

| Prioridade | Caminho | Evidência atual | Condição para concluir |
|---|---|---|---|
| Alta | Papéis e troca de implementação | Papéis de administração/troca no histórico; testes locais rejeitaram chamadas sem papel e uma troca inválida. | Comparar papéis atuais na rede com as contas descobertas e medir invariantes antes/depois de uma troca válida em mais estados. |
| Alta | Assinatura, prazo e contador | O caminho `permit` abriu novos blocos; uma assinatura válida passou no caso local. O sinal do Mythril aponta uso de `block.timestamp` no prazo, esperado para expiração. | Relacionar domínio, prazo, contador, assinatura válida/inválida e estado do proxy em sequências adicionais; exigir diferença repetível antes de tratar como falha. |
| Média | Autorização sem limite e retirada por terceiro | Aprovações sem limite aparecem no histórico; sequências `approve` → `burnFrom` e `transferFrom` percorreram caminhos adicionais sem diferença registrada. | Medir autorização máxima, parcial e zerada antes/depois de pausa e mudanças de papel, em cópia local e com balanços exatos. |
| Média | Pontas do mapa e fontes ativas | Há ligações observadas fora do mapa estático; bibliotecas ativas ainda têm ramos não percorridos. | Associar cada nova saída do bytecode à origem na fonte e distinguir caminho possível de biblioteca não usada. |
| Baixa | Avisos genéricos das ferramentas | O único sinal Slither ligado ao arquivo principal é de versões de `pragma`; o Mythril aponta o prazo de `permit`. | Manter a classificação contextual; reabrir somente com estado e efeito verificáveis. |

## Dez ferramentas

| Ferramenta | Estado nesta rodada | Leitura |
|---|---|---|
| Slither | executado | 114 sinais brutos no conjunto; 1 com referência ao arquivo principal; tipos locais: pragma; compilador local 0.8.27; saída 0 |
| Mythril | executado | 1 sinal(is) bruto(s); tipos: Dependence on predictable environment variable; saída 1 |
| Securify2 | não aplicável | executor legado usa Solidity 0.5.12 |
| Manticore | não aplicável | executor legado sem validação para este runtime |
| Foundry | executado | 18 cenários locais passaram no bloco 26228465; papéis ativos, saldos, emissão, pausa, assinatura e sequências com 32 entradas; cópia local |
| Echidna | executado | duas regras de oferta/saldos em sequências locais, alvo de 200 chamadas; saída 0 |
| Medusa | executado | duas regras de oferta/saldos em sequências locais, alvo de 200 chamadas; saída 0 |
| Certora | indisponível | CLI sem chave; serviço externo não acionado |
| Halmos | concluído | três propriedades locais de oferta, transferência e pausa: concluída |
| Kontrol | pendente | exige propriedade formal e premissas |

O rótulo bruto `High` do Slither não foi convertido em falha alta: os dois sinais altos estão em bibliotecas importadas. O primeiro é o XOR que a própria fonte explica como semente da inversão modular; o segundo é a guarda deliberada de `msg.value` no utilitário de troca. A leitura da fonte não mostrou, por si, um efeito incorreto no token. Avisos de lint do Halmos apontam o harness local, não a implementação publicada.

| Sinal complementar | Pontos | Faixa | Leitura |
|---|---:|---|---|
| Mythril: `block.timestamp` no prazo de `permit` | 24 | baixa | O prazo de expiração usa tempo do bloco; o sinal bruto é esperado nesse desenho. Não foi vista diferença de estado. |

## Pontos do Rust para revisão

Os 30 itens abaixo são dúvidas do mapa. A pontuação cresce quando a posição foi percorrida nesta rodada; não é chance de falha.

| PC | Questão do motor | Visto nesta rodada? | Pontos | Faixa |
|---:|---|---|---:|---|
| 652 | storage key is not a resolved constant | sim | 65 | média |
| 988 | storage key is not a resolved constant | sim | 65 | média |
| 1635 | storage key is not a resolved constant | sim | 65 | média |
| 1679 | storage key is not a resolved constant | sim | 65 | média |
| 2259 | storage key is not a resolved constant | sim | 65 | média |
| 2275 | storage key is not a resolved constant | sim | 65 | média |
| 2456 | storage key is not a resolved constant | sim | 65 | média |
| 2914 | storage key is not a resolved constant | sim | 65 | média |
| 5384 | storage key is not a resolved constant | sim | 65 | média |
| 1773 | jump destination unresolved or invalid | sim | 55 | média |
| 1796 | jump destination unresolved or invalid | sim | 55 | média |
| 1865 | jump destination unresolved or invalid | sim | 55 | média |
| 1899 | jump destination unresolved or invalid | sim | 55 | média |
| 1913 | jump destination unresolved or invalid | sim | 55 | média |
| 1969 | jump destination unresolved or invalid | sim | 55 | média |
| 2171 | jump destination unresolved or invalid | sim | 55 | média |
| 2407 | jump destination unresolved or invalid | sim | 55 | média |
| 2878 | jump destination unresolved or invalid | não | 45 | média |
| 3095 | jump destination unresolved or invalid | sim | 55 | média |
| 3294 | jump destination unresolved or invalid | não | 45 | média |
| 3395 | jump destination unresolved or invalid | sim | 55 | média |
| 3683 | jump destination unresolved or invalid | sim | 55 | média |
| 3991 | jump destination unresolved or invalid | sim | 55 | média |
| 4217 | jump destination unresolved or invalid | não | 45 | média |
| 4913 | jump destination unresolved or invalid | sim | 55 | média |
| 4957 | jump destination unresolved or invalid | sim | 55 | média |
| 5169 | jump destination unresolved or invalid | sim | 55 | média |
| 5185 | jump destination unresolved or invalid | sim | 55 | média |
| 5368 | jump destination unresolved or invalid | sim | 55 | média |
| 5760 | jump destination unresolved or invalid | não | 45 | média |

## Todas as escolhas da cópia local

Pontos aqui medem ganho de caminho e diferença de estado; uma chamada recusada não é uma falha. `Preparação` conta operações locais feitas antes da escolha principal.

| Nº | Família | Preparação | Aplicada localmente? | PCs novos | Ligações novas | Pontos | Faixa | Motivo |
|---:|---|---:|---|---:|---:|---:|---|---|
| 1 | burnFrom | 1 | sim | 30 | 9 | 53 | média | caminho novo observado |
| 2 | burnFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 3 | burnFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 4 | burnFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 5 | burnFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 6 | transfer | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 7 | transferFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 8 | mint | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 9 | burn | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 10 | transferFrom | 1 | sim | 0 | 1 | 27 | baixa | caminho novo observado |
| 11 | transferFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 12 | transferFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 13 | transferFrom | 1 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 14 | transfer | 1 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 15 | pause | 0 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 16 | transfer | 1 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 17 | burnFrom | 1 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 18 | burn | 1 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 19 | transferFrom | 1 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 20 | unpause | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 21 | approve | 0 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 22 | grant | 0 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 23 | revoke | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 24 | permit | 0 | não | 473 | 33 | 80 | alta | caminho novo observado |
| 25 | permit | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 26 | permit | 0 | não | 41 | 5 | 47 | média | caminho novo observado |
| 27 | permit | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 28 | mint | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 29 | approve | 0 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 30 | mint | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 31 | mint | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 32 | transfer | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 33 | grant | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 34 | pause | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 35 | burnFrom | 0 | não | 34 | 2 | 34 | baixa | caminho novo observado |
| 36 | burnFrom | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 37 | burnFrom | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 38 | burnFrom | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |
| 39 | approve | 0 | sim | 0 | 0 | 18 | baixa | operação local sem caminho novo |
| 40 | mint | 0 | não | 0 | 0 | 12 | baixa | chamada não aplicada; caminho conhecido |

## Todos os sinais do Slither

Os 114 sinais brutos são pontuados pelo rótulo da ferramenta, ligação ao arquivo principal e passagem pelo arquivo na medição da fonte. A pontuação não valida o sinal.

| Nº | Detector | Rótulo bruto | Arquivo principal? | Arquivo ativo? | Pontos | Faixa | Arquivo citado |
|---:|---|---|---|---|---:|---|---|
| 1 | incorrect-exp | High | não | não | 37 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 2 | msg-value-in-nonpayable | High | não | sim | 52 | média | @openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol |
| 3 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 4 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 5 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 6 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 7 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 8 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 9 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 10 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 11 | divide-before-multiply | Medium | não | não | 17 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 12 | unused-return | Medium | não | sim | 32 | baixa | @openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol |
| 13 | unused-return | Medium | não | sim | 32 | baixa | @openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol |
| 14 | shadowing-local | Low | não | sim | 10 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol, @openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol |
| 15 | timestamp | Low | não | sim | 10 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol |
| 16 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 17 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/cryptography/ECDSA.sol |
| 18 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol |
| 19 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 20 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol |
| 21 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 22 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 23 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 24 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/Strings.sol |
| 25 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 26 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/SafeCast.sol |
| 27 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 28 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 29 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol |
| 30 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 31 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 32 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 33 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/StorageSlot.sol |
| 34 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/Address.sol |
| 35 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol |
| 36 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 37 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 38 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/Panic.sol |
| 39 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 40 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/Strings.sol |
| 41 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 42 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/Strings.sol |
| 43 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 44 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 45 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/Strings.sol |
| 46 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 47 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 48 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 49 | assembly | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol |
| 50 | assembly | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 51 | pragma | Informational | sim | sim | 26 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol, @openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol, @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20BurnableUpgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol, @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol, @openzeppelin/contracts/access/IAccessControl.sol, @openzeppelin/contracts/interfaces/IERC1967.sol, @openzeppelin/contracts/interfaces/IERC5267.sol, @openzeppelin/contracts/interfaces/draft-IERC1822.sol, @openzeppelin/contracts/interfaces/draft-IERC6093.sol, @openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol, @openzeppelin/contracts/proxy/beacon/IBeacon.sol, @openzeppelin/contracts/token/ERC20/IERC20.sol, @openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol, @openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol, @openzeppelin/contracts/utils/Address.sol, @openzeppelin/contracts/utils/Errors.sol, @openzeppelin/contracts/utils/Panic.sol, @openzeppelin/contracts/utils/StorageSlot.sol, @openzeppelin/contracts/utils/Strings.sol, @openzeppelin/contracts/utils/cryptography/ECDSA.sol, @openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol, @openzeppelin/contracts/utils/introspection/IERC165.sol, @openzeppelin/contracts/utils/math/Math.sol, @openzeppelin/contracts/utils/math/SafeCast.sol, @openzeppelin/contracts/utils/math/SignedMath.sol, src/LatamStable.sol |
| 52 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol |
| 53 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 54 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 55 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 56 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 57 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol |
| 58 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 59 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 60 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 61 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol |
| 62 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol |
| 63 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 64 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20BurnableUpgradeable.sol |
| 65 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 66 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol |
| 67 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 68 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol |
| 69 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 70 | dead-code | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 71 | solc-version | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol, @openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20BurnableUpgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol, @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol, @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol, @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol, @openzeppelin/contracts/access/IAccessControl.sol, @openzeppelin/contracts/interfaces/IERC1967.sol, @openzeppelin/contracts/interfaces/IERC5267.sol, @openzeppelin/contracts/interfaces/draft-IERC1822.sol, @openzeppelin/contracts/interfaces/draft-IERC6093.sol, @openzeppelin/contracts/proxy/beacon/IBeacon.sol, @openzeppelin/contracts/token/ERC20/IERC20.sol, @openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol, @openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol, @openzeppelin/contracts/utils/Address.sol, @openzeppelin/contracts/utils/Errors.sol, @openzeppelin/contracts/utils/Panic.sol, @openzeppelin/contracts/utils/StorageSlot.sol, @openzeppelin/contracts/utils/Strings.sol, @openzeppelin/contracts/utils/cryptography/ECDSA.sol, @openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol, @openzeppelin/contracts/utils/introspection/IERC165.sol, @openzeppelin/contracts/utils/math/Math.sol, @openzeppelin/contracts/utils/math/SafeCast.sol, @openzeppelin/contracts/utils/math/SignedMath.sol |
| 72 | solc-version | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol, @openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol |
| 73 | low-level-calls | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/Address.sol |
| 74 | low-level-calls | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/Address.sol |
| 75 | low-level-calls | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/Address.sol |
| 76 | low-level-calls | Informational | não | sim | 5 | baixa | @openzeppelin/contracts/utils/Address.sol |
| 77 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 78 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol |
| 79 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 80 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 81 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 82 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol |
| 83 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20BurnableUpgradeable.sol |
| 84 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol |
| 85 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol |
| 86 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 87 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol |
| 88 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 89 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 90 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol |
| 91 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol |
| 92 | naming-convention | Informational | não | não | 5 | baixa | @openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol |
| 93 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 94 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol |
| 95 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol |
| 96 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 97 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 98 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PausableUpgradeable.sol |
| 99 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol |
| 100 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol |
| 101 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 102 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol |
| 103 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 104 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20PermitUpgradeable.sol |
| 105 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol |
| 106 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/token/ERC20/extensions/ERC20BurnableUpgradeable.sol |
| 107 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/introspection/ERC165Upgradeable.sol |
| 108 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 109 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol |
| 110 | naming-convention | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 111 | too-many-digits | Informational | não | não | 5 | baixa | @openzeppelin/contracts/utils/math/Math.sol |
| 112 | unindexed-event-address | Informational | não | não | 5 | baixa | @openzeppelin/contracts/interfaces/IERC1967.sol |
| 113 | unindexed-event-address | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |
| 114 | unindexed-event-address | Informational | não | sim | 5 | baixa | @openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol |

## Todos os registros da rede consultados

Cada um dos 236 registros tem pontos de **atenção para revisão**, não uma nota de risco demonstrado. A regra usa tipo de evento e valor frente à oferta do bloco: troca de implementação 78; papéis de administração/troca 70; outros papéis 58; autorização sem limite 56; emissão/retirada 53–76; movimentos comuns 12–63. Um registro conhecido e conciliado pode ter pontuação alta por afetar uma função importante.

| Nº | Bloco | Log | Tipo | Valor bruto | Pontos | Faixa | Motivo | Transação |
|---:|---:|---:|---|---:|---:|---|---|---|
| 1 | 23418962 | 402 | upgraded | — | 78 | alta | troca de implementação; conferir identidade e estado | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 2 | 23418962 | 403 | role_granted | — | 70 | alta | concessão de DEFAULT_ADMIN_ROLE | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 3 | 23418962 | 404 | role_granted | — | 58 | média | concessão de PAUSER_ROLE | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 4 | 23418962 | 405 | role_granted | — | 58 | média | concessão de MINTER_ROLE | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 5 | 23418962 | 406 | role_granted | — | 70 | alta | concessão de UPGRADER_ROLE | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 6 | 23418962 | 407 | initialized | — | 52 | média | início do proxy; conferir sequência de papéis | `0xb96280f5d74523b23f5fd4dca0a549a3846a08d6280a0aad84103b86e58ec7a0` |
| 7 | 23948484 | 652 | role_granted | — | 70 | alta | concessão de DEFAULT_ADMIN_ROLE | `0x0b5f9564aa83a146721fbb954f7b07f4f20aa3842c5595e71b357afe731a24a7` |
| 8 | 24435347 | 326 | mint | 1000000000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x69f73d14ab59088a10952ffb32805caab4c34ebd482a3d50c38a158a382e67a2` |
| 9 | 24442255 | 1236 | transfer | 1000000000000000000000 | 20 | baixa | movimento comum | `0x22867294e70df073c8a72f2c01f65a52ad9a7583659a19d57ab6907d8a7adcb4` |
| 10 | 24442453 | 822 | transfer | 1000000000000000000000 | 20 | baixa | movimento comum | `0x7539ee04379d974d2a0308d5c323863ed4ad5d7cc1d15e5c89f438041099f672` |
| 11 | 24442641 | 843 | transfer | 500000000000000000000 | 20 | baixa | movimento comum | `0x84f02e93b926f15d9781df5040ab8aa638a3924b6cff258e02293c7b899d4d91` |
| 12 | 24519860 | 802 | transfer | 20000000000000000000 | 20 | baixa | movimento comum | `0xf279465eb37d3b409972062a4a469640e01f517a15dba8bd2a7ce76101da77ca` |
| 13 | 24530288 | 493 | role_granted | — | 58 | média | concessão de MINTER_ROLE | `0x30dd3e97cdca3e5d0624882d62af13f2dd556f385b8b53a1f7c0a19e52653674` |
| 14 | 24530644 | 915 | role_granted | — | 58 | média | concessão de MINTER_ROLE | `0xa7c2b09b119a46bc595035d8fce0487fbb6e4bdac68ceeaab326a06c157b8ea0` |
| 15 | 24542716 | 421 | mint | 3655000000000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x9d631e612a4d0dfa848d8f1e010adfe603191cacd3b8ee569ae455a25bad5753` |
| 16 | 24622840 | 367 | transfer | 3655000000000000000000000 | 20 | baixa | movimento comum | `0xbbf4c48674f7c9974480c00df0f38c61cd65e5d202e2580e0a0b99f666f04e7f` |
| 17 | 24634393 | 1589 | transfer | 3655000000000000000000000 | 20 | baixa | movimento comum | `0x449791bfb9629e646de9f7112a51241c6a9d57a3b969dc97b102fb034a3e9118` |
| 18 | 24787223 | 746 | mint | 18300000000000000000000000 | 64 | média | mudança de oferta; conferir papel e balanço | `0xc3295655a22b21fa5e67d40c84e9373005e1aeb85a70952f3439765df9b8020d` |
| 19 | 24950549 | 243 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x2ed24e3b6a536f60b15d16fab3081906218008ba1be0938fe7944f850a8e611c` |
| 20 | 24950580 | 492 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xa1eebededbd31532d0048949fd21f90678f30e26a557fe95d9fbe24476e0c278` |
| 21 | 24950607 | 119 | transfer | 5343058744394882269822680 | 44 | média | movimento relevante frente à oferta do bloco | `0xc74da4e2c5fcfae1a7d508f7492e1cf2103dab7c2b633208449d8c9966836953` |
| 22 | 24950611 | 2 | transfer | 709849810956265147909342 | 20 | baixa | movimento comum | `0x8add1d6d997a2a0ea7eab31fdb66314bd0ac0f4c82dee6bb5ac714f2bd14612c` |
| 23 | 24950612 | 95 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x7a2cacec2f63603dd659e14a62e254edff9b46c2fd45950ffb5abd7b019bfbf9` |
| 24 | 24950614 | 173 | transfer | 543102441586219389078767 | 20 | baixa | movimento comum | `0x48466d98ff7c2da293f0a462b391e9a9477f263aaa5e7b37328fa269e5999cec` |
| 25 | 24950616 | 134 | transfer | 770370430101759385743666 | 20 | baixa | movimento comum | `0xd3224ec375cebefb13c49528f56341dba332fed79c297eeca55526b90f54cae3` |
| 26 | 24950617 | 8 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x848f38d83059e32cbbd5e84dfacf86f875d698ede27ebd3a1cb50573ebe98dd0` |
| 27 | 24950622 | 86 | transfer | 288337915282068382395112 | 20 | baixa | movimento comum | `0x6a79c6bc212eeeb2532bade5ba1024a5f1d6b879056776ec3d4ef07209279a09` |
| 28 | 24950623 | 74 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xbd16ff7eab46f7cd161393da4353edb7d5a379cb8ce8690594f465da5809526b` |
| 29 | 24950627 | 2 | transfer | 242251339254612702498749 | 20 | baixa | movimento comum | `0x30d59efd52a02aa71c55a46d1e893af9e99554cea1c22aa90d50df9a9a647cc6` |
| 30 | 24950629 | 25 | transfer | 381569684178907012209370 | 20 | baixa | movimento comum | `0x371b480d834d40bd5c8cca5e69821006f742a45eed61342966eb09e44a0bd53d` |
| 31 | 24950629 | 105 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xe843162d7fc77accc1f522b08139c8b67ce9eb3dc507e33c790ac5aa5f87e0c3` |
| 32 | 24950630 | 192 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x3221f5c3239d8813ddce3203cd9f6e4f7f1aa644fe357147f319c2cdf8c8da57` |
| 33 | 24950633 | 101 | transfer | 154192103310602591871707 | 20 | baixa | movimento comum | `0x188f0338ac46a996df7c7bc29d98ad641e5305ca6bb0b409485a1ee515ea1931` |
| 34 | 24950633 | 146 | transfer | 28498099065560221637615 | 20 | baixa | movimento comum | `0x8d2c5a49219b9b46e46278ddeabdba8f1a7448ff9530c66ef8050322517f5171` |
| 35 | 24950634 | 280 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x6849087ce522c3db81ce304da6736d6407116e6155488d0d24a371ea398c24af` |
| 36 | 24950634 | 281 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x37754c356c29861848d6234bdc708576611896f583ca0b1999e696e2de720dc0` |
| 37 | 24950639 | 36 | transfer | 60562834813653175624687 | 20 | baixa | movimento comum | `0xabd09eb122669a9659d0d81b55c88bfb4d4f402c34b0c98256d19dfc194d0fea` |
| 38 | 24950639 | 39 | transfer | 60562834813653175624687 | 20 | baixa | movimento comum | `0xabd09eb122669a9659d0d81b55c88bfb4d4f402c34b0c98256d19dfc194d0fea` |
| 39 | 24950640 | 12 | transfer | 626476126271242268494054 | 20 | baixa | movimento comum | `0x6066040fa0220d58d0f392d36a831bd80893c91cde911e9989cf5085734c1d55` |
| 40 | 24950640 | 15 | transfer | 626476126271242268494054 | 20 | baixa | movimento comum | `0x6066040fa0220d58d0f392d36a831bd80893c91cde911e9989cf5085734c1d55` |
| 41 | 24950641 | 129 | transfer | 630270863683453423852322 | 20 | baixa | movimento comum | `0x48b5cc7aed9e3fe7632bc2859116c6426c23b54913b1674c7da0451f9686d20d` |
| 42 | 24950642 | 97 | transfer | 181688504440959499999999 | 20 | baixa | movimento comum | `0x8f70dcae9faf83bb8db24f82455c11d2d2f1765854af632d29bf6926539a844c` |
| 43 | 24950642 | 100 | transfer | 181688504440959499999999 | 20 | baixa | movimento comum | `0x8f70dcae9faf83bb8db24f82455c11d2d2f1765854af632d29bf6926539a844c` |
| 44 | 24950643 | 47 | transfer | 288337915282068382395112 | 20 | baixa | movimento comum | `0xce30cee65bb46907c464a383e77f2173fda2b87729c8238e4c00f5e9fe9dafec` |
| 45 | 24950643 | 50 | transfer | 288337915282068382395112 | 20 | baixa | movimento comum | `0xce30cee65bb46907c464a383e77f2173fda2b87729c8238e4c00f5e9fe9dafec` |
| 46 | 24950644 | 2 | transfer | 217522698216083951821187 | 20 | baixa | movimento comum | `0x123c400d1a9034c4c111c73934657d26dc4fd8ad27f48a153d7fbd6ecc53d93f` |
| 47 | 24950645 | 30 | transfer | 368589925064098473029586 | 20 | baixa | movimento comum | `0xde165e65787519a5b961d3c90c70b76e686b18695baa6f16ceba19f88c241698` |
| 48 | 24950646 | 75 | transfer | 158655868332475013011878 | 20 | baixa | movimento comum | `0x51794eec7e2d5e036813979be8786e33b79ff8abdb940718297b0a335f9184d3` |
| 49 | 24950647 | 141 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xa73b0354ee6ffd4108fa00a2216441643374a9a508aed5c0195dc6961cb3da75` |
| 50 | 24950649 | 842 | transfer | 158655868332475013011878 | 20 | baixa | movimento comum | `0x0481b200a82c8e9e5064d73bef4f5293b77f837a25ccf271598664d9ce54993a` |
| 51 | 24951125 | 82 | transfer | 28498099065560221637615 | 20 | baixa | movimento comum | `0x0476265a1abb0d0250fde513379912602b7cba8db7ef7b2cd13473709df1b647` |
| 52 | 24952024 | 180 | transfer | 522782028374701064901293 | 20 | baixa | movimento comum | `0x5566c9111cba466830661cac1d9ca8abb6c2a030771b39b30ed1dbfd7517af1a` |
| 53 | 24952380 | 0 | transfer | 381569684178907012209370 | 20 | baixa | movimento comum | `0x9808f15d4970fc476c33c6b6b285c034230c2c103bb9c97737bc8d2f364db1c2` |
| 54 | 24967965 | 87 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xfb0dc74f33f424ae7e9212731214b03daa4f81026f90c41fe8536a336afde1fc` |
| 55 | 24967968 | 281 | transfer | 217522698216083978695250 | 20 | baixa | movimento comum | `0x121eb712eb1e357bad84b573815bdc89a78e5d9090c60ca39654924cbab05b88` |
| 56 | 24967968 | 284 | transfer | 217522698216083978695250 | 20 | baixa | movimento comum | `0x121eb712eb1e357bad84b573815bdc89a78e5d9090c60ca39654924cbab05b88` |
| 57 | 24995207 | 411 | role_granted | — | 58 | média | concessão de PAUSER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 58 | 24995207 | 412 | role_granted | — | 58 | média | concessão de MINTER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 59 | 24995207 | 413 | role_granted | — | 70 | alta | concessão de UPGRADER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 60 | 24995207 | 414 | role_granted | — | 70 | alta | concessão de DEFAULT_ADMIN_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 61 | 24995207 | 415 | role_revoked | — | 58 | média | retirada de PAUSER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 62 | 24995207 | 416 | role_revoked | — | 58 | média | retirada de MINTER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 63 | 24995207 | 417 | role_revoked | — | 70 | alta | retirada de UPGRADER_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 64 | 24995207 | 418 | role_revoked | — | 70 | alta | retirada de DEFAULT_ADMIN_ROLE | `0x9260d65e8b39566ac5a44229b6ae4c8ab6fb4ea344f1c7d02ae1e77c78d0b90a` |
| 65 | 24996394 | 528 | transfer | 1100155864252147627722 | 20 | baixa | movimento comum | `0xd7c1a8932a481c1193c5059729b88054104e09d9fe8786a669dacb22c7c21217` |
| 66 | 24996410 | 334 | transfer | 600000000000000000000 | 20 | baixa | movimento comum | `0x8b6fdef260359bed7debc80067cf0bf95d894012e166a1c3b79dcdbfd8329588` |
| 67 | 24997268 | 397 | transfer | 23370960284080257951573 | 20 | baixa | movimento comum | `0x26ee46adf9d6c699f10d93e4bdaf0d1761db8805ee6e74b27bb248ba335be002` |
| 68 | 24997274 | 134 | transfer | 13000000000000000000000 | 20 | baixa | movimento comum | `0x8c22cca5a0bbba298248e2e02c013e73d70a0efa727fab77a0f94468785524d8` |
| 69 | 25003542 | 545 | mint | 68584200000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0xac593386a6f0e8420febd5a5572418099f846d3b518e96f536722ef8d47933ae` |
| 70 | 25003552 | 750 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x1ab0705526fafedd8853885a56fc9902ca80a741a453173badb719f0528b2d4a` |
| 71 | 25003558 | 615 | transfer | 68584200000000000000000 | 20 | baixa | movimento comum | `0x342bdd14b6c702088941efc5767bd5bc9a6257134a91b1fa2c6695be87dfa0dd` |
| 72 | 25003603 | 795 | mint | 638998470000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x2fc123d1f6e9a2673db028edf317e5c1091d528edb5030b87dd920738451d25c` |
| 73 | 25003618 | 757 | transfer | 638998470000000000000000 | 20 | baixa | movimento comum | `0x96629b95317e42bab462ce9c8e9c1f59d796a0782a6b228ed18d485d168c1961` |
| 74 | 25015874 | 0 | transfer | 140099566418305961891344 | 20 | baixa | movimento comum | `0x2794cf00e91d1b0398ef4da3569c3cf2c0e4d2dcaeb33776ad8b4a3c183048f9` |
| 75 | 25022623 | 908 | transfer | 2867399017218212125093585 | 20 | baixa | movimento comum | `0xd765aebf2d97e87fc5617f0807060b510a69166148578366ce434b38fa15f80d` |
| 76 | 25022712 | 99 | transfer | 9783692088621413648762443 | 44 | média | movimento relevante frente à oferta do bloco | `0xb1961c3ed0fecacc30939435fc0f017a2fdabe7373db284e40aeafd5c63c90bf` |
| 77 | 25137290 | 127 | transfer | 50000000000000000000000 | 20 | baixa | movimento comum | `0xeda2b90b9def3eae096cc19473ce16bf617d593e056ccfbdf2d7340f99926ba9` |
| 78 | 25137294 | 85 | transfer | 50000000000000000000000 | 20 | baixa | movimento comum | `0xe5c3970f609e5cd41f7412339c73fd476958c897a415e3bdfe80aa6bd62188e1` |
| 79 | 25210116 | 175 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x30f4f824801fa5cca76e03faca71306a1b56bd0d0aef91213a4ed44cc0163f17` |
| 80 | 25210117 | 183 | transfer | 54355580741650000000 | 20 | baixa | movimento comum | `0xee40a0c21ce6aaac010340d543697aa5074292f327162310fb6a73bb3c761734` |
| 81 | 25210117 | 184 | transfer | 10816760567588350000000 | 20 | baixa | movimento comum | `0xee40a0c21ce6aaac010340d543697aa5074292f327162310fb6a73bb3c761734` |
| 82 | 25222818 | 802 | approval | 50000000000000000000000 | 30 | baixa | autorização finita | `0xccd8cf9a4573b06df8dad7ce9c6953adc24acb85c0b4a8e87b9dad8e4e6b96e0` |
| 83 | 25222818 | 805 | transfer | 50000000000000000000000 | 20 | baixa | movimento comum | `0xccd8cf9a4573b06df8dad7ce9c6953adc24acb85c0b4a8e87b9dad8e4e6b96e0` |
| 84 | 25223331 | 356 | transfer | 6040648000000000000000000 | 44 | média | movimento relevante frente à oferta do bloco | `0x2905d772d2c02bc2b9af29d45b79eedc2a6b4e29e4b3a1be0a1a1d34b6396b10` |
| 85 | 25223599 | 871 | transfer | 6040648000000000000000000 | 44 | média | movimento relevante frente à oferta do bloco | `0x65bcda2a0af488fc36263246014c664b73c08b8270d11d6772d3d3a8140c4c60` |
| 86 | 25252594 | 429 | transfer | 9520000000000000000000 | 20 | baixa | movimento comum | `0x7589a8aa7de631f52b7b3deb504f161f6fa7e710e94955fa01ff911a3b4bc6e0` |
| 87 | 25252842 | 662 | transfer | 3700000000000000000000 | 20 | baixa | movimento comum | `0x1650dbe1930b6476faa839f0f4f2fffaf5f726074c94ce9b0f3a0938e31672d3` |
| 88 | 25283068 | 153 | approval | 0 | 18 | baixa | autorização zerada | `0x6f46ea4510f774346d84905515faf9892bde72a720495ecde348fdacb5df41dc` |
| 89 | 25330359 | 742 | approval | 5000000000000000000000000 | 30 | baixa | autorização finita | `0xb931e8d8c7ff71f9ae93fb4c8a63c1e9fb9fd0b4bda39d5e5081723234f74be0` |
| 90 | 25377438 | 732 | transfer | 360264612240751464215321 | 20 | baixa | movimento comum | `0x2401f7b8c9bfa9cb0ed924615f7d67aa6b0ba1d798d3d4e096d4f2aab8470b08` |
| 91 | 25377448 | 168 | transfer | 360264612240751464215321 | 20 | baixa | movimento comum | `0xb32c10389838a60cce48ae5dae8ac40507be336a4b5321fb4eda39fc196929ec` |
| 92 | 25377632 | 509 | transfer | 715622291336876039484922 | 20 | baixa | movimento comum | `0xff31b12125e42ddf86f0d1b43e1e47ed897426db771efc681b8410809f8ff66c` |
| 93 | 25377638 | 1004 | transfer | 715622291336876039484922 | 20 | baixa | movimento comum | `0x877b4810e37b939991923c34f3951dd1cd9b179b859be8ad2e80b886e459c1fe` |
| 94 | 25378743 | 523 | transfer | 1075886903577627503700243 | 20 | baixa | movimento comum | `0xe3738137c57a7b814c65899d68a37ff0a2983738c359de835748a28c5defd466` |
| 95 | 25379980 | 1106 | transfer | 711055182781028648864556 | 20 | baixa | movimento comum | `0x4f4f8f47ba526869573a1fcd27e2d6ebfc2889ae5f51de50fcd94342b3679525` |
| 96 | 25379987 | 283 | transfer | 711055182781028648864556 | 20 | baixa | movimento comum | `0x87f1f2db85a97a0d2b8bf3af26681066513da26a37fe5b064b231fd55f749257` |
| 97 | 25380219 | 127 | transfer | 704799189824966428148356 | 20 | baixa | movimento comum | `0x6c4d767cfdb4bf15ff30de7ef7670dcf51c597443c8bdb3d6f4977afaa1f4992` |
| 98 | 25380227 | 264 | transfer | 704799189824966428148356 | 20 | baixa | movimento comum | `0x02b8b9e6d20271b7e52e95ea32c46398895ca769833828971a0a6fd351d28e61` |
| 99 | 25380507 | 52 | transfer | 700906323107795882301039 | 20 | baixa | movimento comum | `0xf496578764da3a6b40d31c422880f48275427d8bfae708378e5acf86022ba492` |
| 100 | 25380510 | 375 | transfer | 700906323107795882301039 | 20 | baixa | movimento comum | `0xbc72a4c72d96570e233f2c278840a2923d80baa31d578add19e0866cc6631efd` |
| 101 | 25380609 | 622 | mint | 2042301960000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x6389a30696d44795223ae61388b77e49079f0745013e0bfacb79e848ccddc87d` |
| 102 | 25380613 | 859 | approval | 9007199254740991000000000000000000 | 30 | baixa | autorização finita | `0xd490bfb3511d8740383721a1e7618b83da3bee596ef939e51fd46cd75ab34290` |
| 103 | 25380615 | 213 | transfer | 2042301900000000000000000 | 20 | baixa | movimento comum | `0xe5b8adba4f3882c3b24ce0b15babd5c3f1d2f5de073cc7c8e93d4acc4e001a27` |
| 104 | 25380857 | 420 | transfer | 710236980935902766817874 | 20 | baixa | movimento comum | `0x48401b6b66f3723024fd8ac100f5c02454d85137b8ffd2941e58dd10056c3619` |
| 105 | 25380860 | 441 | transfer | 710236980935902766817874 | 20 | baixa | movimento comum | `0x66eed50a8ba9f7332c77c16755a20abbf21d7ed458c8c1674d574a7f45a7e6fa` |
| 106 | 25381214 | 96 | transfer | 705360702246124414078275 | 20 | baixa | movimento comum | `0x690b5f1ca1bb5cc2853f4ab94d38afb8a69f9054cf3cabcf78133894d9534bad` |
| 107 | 25381219 | 704 | transfer | 705360702246124414078275 | 20 | baixa | movimento comum | `0xea31af974fa5b20177fe5f2b45c3be4e3b20508bc2aa0d8c62c4d9ccc65e4871` |
| 108 | 25381899 | 454 | transfer | 699419005405588266905594 | 20 | baixa | movimento comum | `0xbb0b5f88f87fa4b541b47460055263ea02de9dfd913239fdc225641f36f27883` |
| 109 | 25381905 | 322 | transfer | 699419005405588266905594 | 20 | baixa | movimento comum | `0x9732d58948d3ad0a27619a157d9bff0603b2cc3c319c248db40c104eda34f243` |
| 110 | 25383872 | 327 | transfer | 344415756725519766418776 | 20 | baixa | movimento comum | `0x15911ab14d2751e51afe676ab1dcdffe1750fc5a796b82fb918156b0626c7ba1` |
| 111 | 25383921 | 293 | transfer | 344415756725519766418776 | 20 | baixa | movimento comum | `0x40da8e369ceae6bd047f0c3960c9fa447e19832859ba6c0730d89b89c059fa21` |
| 112 | 25385864 | 772 | transfer | 4576193141026926173534470 | 20 | baixa | movimento comum | `0xbc4111faf0c8ed114fdb341384e3ba21c16f24c03417436263b9ac37b3134881` |
| 113 | 25388422 | 1049 | transfer | 3100000000000000000000 | 20 | baixa | movimento comum | `0xc3145f89b4add07ba31bc8e6a51f1f6daa52163bdc088e9e2654de9363b18dcd` |
| 114 | 25388552 | 1090 | transfer | 6800000000000000000000 | 20 | baixa | movimento comum | `0xa92c87caa7d3cb41b5851b7a41a32f8020ef87a8ce460acdc5e3e239f5131221` |
| 115 | 25388711 | 707 | transfer | 3100000000000000000000 | 20 | baixa | movimento comum | `0x43e54f31591573d33699a519a2c1d5bd6bdd4dd44809e9b084ca1459028acd3f` |
| 116 | 25441703 | 462 | transfer | 691529457383636000784057 | 20 | baixa | movimento comum | `0x7890cd6beff38b1c9259b81c669d2fc119081d3399d2313660ce17e8e321e423` |
| 117 | 25441708 | 124 | transfer | 691529457383636000784057 | 20 | baixa | movimento comum | `0x169eeec67fb9535f182e3d64a45e21c48e8ebd5a82a34e3cd341cd53296ba9a1` |
| 118 | 25443217 | 1149 | transfer | 691529457383636000784057 | 20 | baixa | movimento comum | `0x6f815ca232026c03bf859dec32a8f7d6ae45c528dc5dae70ac8d65cd38d10087` |
| 119 | 25447756 | 301 | transfer | 515223471679584694602341 | 20 | baixa | movimento comum | `0x02fd988847d2c8507ccda7dfdad251cb9885f8fa730a45d243f38908e5414d48` |
| 120 | 25447762 | 1660 | transfer | 515223471679584694602341 | 20 | baixa | movimento comum | `0x3e2cf63b25fee1d362ddb8f10ba2de2b321ce8b6fa8a7d12abd3176dda5b0b19` |
| 121 | 25450377 | 1308 | transfer | 515223471679584694602341 | 20 | baixa | movimento comum | `0xf2083265b4f2f3e9b6797e32fc4b865e7d6a9da21777c0781f12b5aa4cfea532` |
| 122 | 25453405 | 208 | transfer | 511926131057907092596588 | 20 | baixa | movimento comum | `0xaaf27489c3814be8c2d7554bec30d4df43bf4dfa9e9649cca0a0b6f44eac555d` |
| 123 | 25453412 | 150 | transfer | 511926131057907092596588 | 20 | baixa | movimento comum | `0x4804b8d757181ff152ca7f1d5e53e4ea34f71b4f78c7840a96b446753cb322c6` |
| 124 | 25453956 | 2463 | transfer | 511926131057907092596588 | 20 | baixa | movimento comum | `0x8ca2e56facccd305e090a47a3e74923e7c8b171f8711a630fe6958a69017aff2` |
| 125 | 25456315 | 119 | transfer | 203878455830033620358486 | 20 | baixa | movimento comum | `0x6eb09c48fe6eac820fdac400172f74ecd4f566f019040c5355314df430b5b08a` |
| 126 | 25456321 | 428 | transfer | 203878455830033620358486 | 20 | baixa | movimento comum | `0xfda637b6606aa91dac37afb335fc469fededcbf0b6feb8d66b7ea4a41d53c77a` |
| 127 | 25457586 | 384 | transfer | 203878455830033620358486 | 20 | baixa | movimento comum | `0x8d047a1f779cdbb0e7139bd2ba7296464c026b8f00bf90e95d7bfc1a5366734e` |
| 128 | 25461235 | 203 | transfer | 180893094583362558635748 | 20 | baixa | movimento comum | `0x0c6188efb7ca591648cf23a69f982921c4b5bf26482aaa1deedced45d9d46dab` |
| 129 | 25461240 | 230 | approval | 18099893154583362558600000 | 30 | baixa | autorização finita | `0xcba77ceefd6e58620c5f216fc0566884f62b8f99a59dc12f982ecadd13244cdc` |
| 130 | 25461242 | 123 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0x1ba5efc125055fcf17a4e807d33cafebbe3c689e5304990ee495cd3abcf78d7e` |
| 131 | 25461242 | 124 | burn | 180836154583362558600000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x1ba5efc125055fcf17a4e807d33cafebbe3c689e5304990ee495cd3abcf78d7e` |
| 132 | 25477966 | 464 | transfer | 275100719630260177874978 | 20 | baixa | movimento comum | `0xc019a9aea02a1a3e432a9f96e6b64e844888ac74b78decda561e01df87103eca` |
| 133 | 25477968 | 216 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0x59aec68ab1283c85ef835a6e5b3e2acf4a1cabaa52e80173500b32178df47263` |
| 134 | 25477968 | 217 | burn | 275043719630260170000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x59aec68ab1283c85ef835a6e5b3e2acf4a1cabaa52e80173500b32178df47263` |
| 135 | 25497422 | 404 | transfer | 673606879180670085917425 | 20 | baixa | movimento comum | `0xce39b4d9af8180c32fb69a8358dc2a7947a0d718132fafa45d62c25847c643a5` |
| 136 | 25497427 | 299 | transfer | 673606879180670085917425 | 20 | baixa | movimento comum | `0x67189095d6c287a6db2c2bb9c31f1f010f967c87efcebbddb8ba93c5e9c3e4da` |
| 137 | 25500572 | 656 | transfer | 673606879180670085917425 | 20 | baixa | movimento comum | `0x797e0b358898fd879706568cf80c151e2d1c7da28c7c22dc26fbd0fd3d70ecc6` |
| 138 | 25503648 | 938 | transfer | 997536594917043752883431 | 20 | baixa | movimento comum | `0x586961eed389b0aa5b4809c7cadd5e673a6f91227be07ec984d5318b2b27ba45` |
| 139 | 25503656 | 539 | transfer | 997536594917043752883431 | 20 | baixa | movimento comum | `0xe3e37feea16e606346e8f851428f41c69753e40b1ff7ea0fe2af99193e38eeab` |
| 140 | 25504160 | 483 | transfer | 997536594917043752883431 | 20 | baixa | movimento comum | `0x653c53a689d63ad63423583197f9310fc2dea539bf8a9fa72011c9ae1e0e136c` |
| 141 | 25524314 | 919 | transfer | 660421022462484737082748 | 20 | baixa | movimento comum | `0x5e2bb545a5beb458dc726c03594b53aa8791b3e86da045b98f0582035a4aa152` |
| 142 | 25524322 | 638 | transfer | 660421022462484737082748 | 20 | baixa | movimento comum | `0x13dac80c29e0e5fc6629b892e00e7d5792a62e7a898aad98ac66c14476461226` |
| 143 | 25524817 | 1966 | transfer | 660421022462484737082748 | 20 | baixa | movimento comum | `0xd8b79d6691f67f0f8aa89830f1848ef594e3f5d92d939e9eaf8ef89254902d95` |
| 144 | 25535309 | 257 | transfer | 487569334492672127623219 | 20 | baixa | movimento comum | `0xc1bb884678394c28413c69e64b5dae8ad0620111cca0c70cda97ea1c6eb5bc12` |
| 145 | 25535311 | 90 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0x955dc1528e495413e7e62ed4c527bc10bb410ebaf9bca6646cded9b70f2c0f90` |
| 146 | 25535311 | 91 | burn | 487512334492672135533900 | 53 | média | mudança de oferta; conferir papel e balanço | `0x955dc1528e495413e7e62ed4c527bc10bb410ebaf9bca6646cded9b70f2c0f90` |
| 147 | 25569316 | 577 | mint | 519227347094300000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0xabed035a1164a79d872d61f151a3b98add991ec7fa79768d15e09d057dad782e` |
| 148 | 25569320 | 431 | transfer | 519227347094300000000000 | 20 | baixa | movimento comum | `0xf9f7530cc8781cd936b91c8e6e2cce9f5584c5e6b1284fd775985b6168a99c50` |
| 149 | 25574888 | 563 | transfer | 190300049551867888419586 | 20 | baixa | movimento comum | `0x6672fc621d32fe9df8266de2ab3909f8e7bbd1761d45a5cb553edae776ed1478` |
| 150 | 25574891 | 605 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0xb300471df26b401b2036272902e839fde97ae5067cf57820eddca3508c6ae56a` |
| 151 | 25574891 | 606 | burn | 190243049551867888000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0xb300471df26b401b2036272902e839fde97ae5067cf57820eddca3508c6ae56a` |
| 152 | 25578296 | 335 | transfer | 3223487661835636822503 | 20 | baixa | movimento comum | `0xe21b03aa3624ae455e006a0d4bdab5f04f9c9d9f8a77528d58d305360033bcda` |
| 153 | 25578322 | 200 | transfer | 3223487661835636822503 | 20 | baixa | movimento comum | `0x60f074d1db878a4100951f25b6962c0b2d4edf28722be4ad688cfa26dec2d9aa` |
| 154 | 25578668 | 67 | transfer | 10162490038624300524815 | 20 | baixa | movimento comum | `0x9ab5d932ea4fb3ee46eddf5470313567ca3cb28c90c5ab57a8404170ba11d6fd` |
| 155 | 25578692 | 1257 | transfer | 3000000000000000000000 | 20 | baixa | movimento comum | `0x0028eaa05b83258a2d1ca4e133dff9e134466b994ad0912301e0771b42aea1ef` |
| 156 | 25578810 | 611 | transfer | 10162490038620000000000 | 20 | baixa | movimento comum | `0x799d314dec01cfd57efcdf23e74fb81b437c9ddefce0b9541faba804f1619fdc` |
| 157 | 25579644 | 603 | transfer | 13162490038620000000000 | 20 | baixa | movimento comum | `0xc9a99b0b07e97cd892db0d3b3320d4125de4faca2ff2ba7bd39361c2be92f29f` |
| 158 | 25588713 | 562 | transfer | 16597636117588017791538 | 20 | baixa | movimento comum | `0xee88ef166609b879f66297626d569d92e56d9444eb504275a9e591148b63a758` |
| 159 | 25588766 | 686 | transfer | 172459450288408819138624 | 20 | baixa | movimento comum | `0x90946cdac8f47d5fed4ea75c47d20f07007f7b00fb769ec6c29192adcfeb8bef` |
| 160 | 25591330 | 233 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x3fb97d61fbff810eac719109adf1c0b808181b5f35b0ff0940f8acbedce3d851` |
| 161 | 25591330 | 234 | transfer | 189280574067830000000000 | 20 | baixa | movimento comum | `0x3fb97d61fbff810eac719109adf1c0b808181b5f35b0ff0940f8acbedce3d851` |
| 162 | 25591683 | 206 | transfer | 185567292880783554288256 | 20 | baixa | movimento comum | `0x04dc447d0f4c3586dbb9b11d55a39d49e51cd0fa5ef27ea6892c8b37ceaa7ee4` |
| 163 | 25591711 | 537 | transfer | 185567292880783554288256 | 20 | baixa | movimento comum | `0x50bb3a11cfa6d5b5b7689001100c62700525d1e5631436e20509b0c0a96cdc6d` |
| 164 | 25592058 | 225 | transfer | 190194678529594104766042 | 20 | baixa | movimento comum | `0x75efe36fe25b700f9656e622230bc87282733e1bcea0e7078bbfb1dc13ca48a0` |
| 165 | 25592090 | 247 | transfer | 190194678529594104766042 | 20 | baixa | movimento comum | `0xb651aac5b1986350570b8e9e02137ae39f163a55118b0ba034ab1087adebaf4c` |
| 166 | 25592910 | 634 | transfer | 191344958927412267569584 | 20 | baixa | movimento comum | `0xe152173b36964ad8711136c5cc0e26e520730562a4a3ca785f4820eb4e43cd79` |
| 167 | 25593845 | 742 | transfer | 375761971410377659054298 | 20 | baixa | movimento comum | `0xc420043075ba6fe54991bd71dc42a44121c420b1c126b887e9b508486f06fe97` |
| 168 | 25601144 | 880 | transfer | 190000000000000000000000 | 20 | baixa | movimento comum | `0xf59cc2e04df0bde8b06acb3dd77a43d402832e308f614e3f150d9b7805d8666f` |
| 169 | 25601155 | 172 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0xdd6693c3508aeafcdf2815eb1d0a92821e655f041104727537a3c85bb966fb1f` |
| 170 | 25601155 | 176 | transfer | 1344958927412267569584 | 20 | baixa | movimento comum | `0xdd6693c3508aeafcdf2815eb1d0a92821e655f041104727537a3c85bb966fb1f` |
| 171 | 25603994 | 191 | transfer | 5000000000000000000000 | 20 | baixa | movimento comum | `0xabe49faf2e9c68d355392811f9dacd3729205208548186963e3c92037c4eb50c` |
| 172 | 25604154 | 462 | transfer | 5000000000000000000000 | 20 | baixa | movimento comum | `0x9bd4c22fa557dda5b537235d29c497a4d1b69ea3c3d3aa36d6d5ff848e7b4caf` |
| 173 | 25604807 | 1008 | transfer | 313510834309059907814544 | 20 | baixa | movimento comum | `0x901a933786824cb11cd11c31a043f32823211936e9838bacf62fd5c11cb98d8e` |
| 174 | 25604816 | 69 | transfer | 2510834309059907814544 | 20 | baixa | movimento comum | `0x630674271abe7cba3be06211725380e54a03d5d0ca80c63753d25df562ff8be0` |
| 175 | 25607015 | 297 | transfer | 97166498919376008774420 | 20 | baixa | movimento comum | `0x74684fff4ccd287360e35c5a126724a723e72713caf4470202da694575f3718a` |
| 176 | 25607340 | 228 | transfer | 1000000000000000000000 | 20 | baixa | movimento comum | `0x62d9826b39395b69873b5b6ee6d7368057a589a3e40bf84d3681d74387b9fc53` |
| 177 | 25608446 | 1034 | transfer | 190000000000000000000000 | 20 | baixa | movimento comum | `0xddabd53cc19e039fbf2a9f7029807cda106cf3c14cb105af5b477365acacfeb1` |
| 178 | 25609426 | 131 | transfer | 407166498919376008774420 | 20 | baixa | movimento comum | `0x1f3f133f1224ea90d8bbfdebc53955136e819e5cf3e616815769818da848b47c` |
| 179 | 25610982 | 431 | transfer | 186547056068514681368177 | 20 | baixa | movimento comum | `0x9b544deba4be1991edd6017d161660b5dd0cbab37e82ff74cc1affb973ab76bb` |
| 180 | 25611189 | 253 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x710a2090f00ffb6c3ca9739b6b13b6300db55a88b1bb0f3846f88046833ac6db` |
| 181 | 25611189 | 257 | transfer | 547060000000000000000 | 20 | baixa | movimento comum | `0x710a2090f00ffb6c3ca9739b6b13b6300db55a88b1bb0f3846f88046833ac6db` |
| 182 | 25611196 | 336 | transfer | 185999996068514681368177 | 20 | baixa | movimento comum | `0x68e9b4c4645dc04df71d569abe2881544558569212b10dacec8be2924e62d995` |
| 183 | 25612519 | 75 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x2f222fc961708391e6794a4a1a6379194027a45fd22b02367b0447522e0f15f9` |
| 184 | 25612519 | 78 | transfer | 185999996068514681368177 | 20 | baixa | movimento comum | `0x2f222fc961708391e6794a4a1a6379194027a45fd22b02367b0447522e0f15f9` |
| 185 | 25615346 | 1287 | transfer | 407166498919376008774420 | 20 | baixa | movimento comum | `0x317c65b487303bfeb2c0391e08f4e7312d9eb5302c3f262ba176a0782588a7b0` |
| 186 | 25629378 | 978 | transfer | 409022471182787576125154 | 20 | baixa | movimento comum | `0xb4ac6e53bff8e725a8655f26a5c6cfe6e360db00823d0dd18c5f2f0f76093cda` |
| 187 | 25629422 | 128 | transfer | 409022471182787576125154 | 20 | baixa | movimento comum | `0x5d2ad3dd5826e009d8e71133e55319a05d6e482c3da8ec81d772e7bf6925248c` |
| 188 | 25634277 | 846 | transfer | 410022471182787576125154 | 20 | baixa | movimento comum | `0x1edcce78e0ab6245f126ad85dd11bb0ac53afe1ed1b3b1b90456ceb28ee1b89d` |
| 189 | 25658398 | 1164 | transfer | 410022471182787576125154 | 20 | baixa | movimento comum | `0x6db6524a318735afd3e731bd129af0810e6911ea50b3d07e20bd6729363112ee` |
| 190 | 25658471 | 163 | transfer | 5000000000000000000000 | 20 | baixa | movimento comum | `0x5e4013bb924a188ae20b3a5db98f63c0fc8af47cee87a843c4579d17ed5f379a` |
| 191 | 25690888 | 1036 | transfer | 2000000000000000000000 | 20 | baixa | movimento comum | `0xb81267270ba61430314104de4809496244ccf860282ea442cc1b92ab6d01f52a` |
| 192 | 25765016 | 390 | approval | 13600000000000000000000 | 30 | baixa | autorização finita | `0x8254b4881b5187672a3751ac9b37132921d498079798b2e386663aaedda02d63` |
| 193 | 25765019 | 180 | transfer | 13600000000000000000000 | 20 | baixa | movimento comum | `0xb25b59688e4f4f0c3eef9a7c3d82658a36d2d7577e9a400d20388dd71238075f` |
| 194 | 25834570 | 290 | mint | 7600000000000000000000000 | 64 | média | mudança de oferta; conferir papel e balanço | `0xfd87e0da5a641f70999aae8630025f35fc10f5284420c3d0bccd36b814749a7b` |
| 195 | 25841394 | 950 | transfer | 7600000184201916206508462 | 44 | média | movimento relevante frente à oferta do bloco | `0x3b59e83b2f6d53cc80005b2433d440c6c8c9b4886da4a692d250e40e471d4421` |
| 196 | 25848217 | 620 | transfer | 18220504052113872289461 | 20 | baixa | movimento comum | `0x9fe8f57da8af9f2c3983488575ec24eafafcfab85872a1a963a659f04edfccae` |
| 197 | 25856253 | 476 | mint | 126800000000000000000000000 | 76 | alta | mudança de oferta; conferir papel e balanço | `0x189ab28158ab4f4ff102d8ccdc023917a1c9618fe71c10842bc23c583eea867e` |
| 198 | 25856537 | 120 | transfer | 20905958273579269664464 | 20 | baixa | movimento comum | `0x0fd5bc13a1eab7c9744d6b1a72d3ed2293736c18c598f6653a543c0f1c84aa4b` |
| 199 | 25856589 | 620 | transfer | 126818220504052113872289461 | 63 | média | movimento grande frente à oferta do bloco | `0xc22dd294d76964c778cedd5516ad8a49dcbda5107ca245e8ce87f2b21bf87ef9` |
| 200 | 25856687 | 594 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x12f03f961ee77ed3fcf1ff2f3033dfd3a8d8c761e45fb841a14e398dbd4420d0` |
| 201 | 25856695 | 82 | transfer | 8072051389208039443890111 | 44 | média | movimento relevante frente à oferta do bloco | `0xf4a73cc8c44ab2f65af2fe563979c687bdd80e25a44cb30050173da8b1839cd8` |
| 202 | 25856814 | 253 | transfer | 126367075257319569892146671 | 63 | média | movimento grande frente à oferta do bloco | `0x64363353a2327378518ff6dcbb9c145259a82814140164eea4e447b49c9b52b5` |
| 203 | 25884001 | 44 | transfer | 500000000000000000 | 20 | baixa | movimento comum | `0x230c588aecf139396780382341f15e1180951d1611be93589e850866c8f93f40` |
| 204 | 25891043 | 545 | transfer | 3200000000000000000000000 | 20 | baixa | movimento comum | `0x1f2ce7c3a2dae3e6a50a5e6fa8e08a295b06bb8dcacae46a2876ee2706f154e0` |
| 205 | 25905450 | 39 | transfer | 1592174966793700702550211 | 20 | baixa | movimento comum | `0x4834e9e1ac93589dc410d707f7c5d2201c86619d4f394d35b55cd1ce26942a97` |
| 206 | 25905456 | 340 | transfer | 1592174966793700702550211 | 20 | baixa | movimento comum | `0xfada071fa4e0a352239e2c0ced518b816c9c6b0ac1378e4acb805c8cce2d69dc` |
| 207 | 25905953 | 192 | transfer | 1592174966793700702550211 | 20 | baixa | movimento comum | `0xd124e90ffcb0f4959f1bd76723c9fcfae00ba7d5fe329e2996c8760b5219a39e` |
| 208 | 25928284 | 336 | transfer | 1902364797603184751334697 | 20 | baixa | movimento comum | `0xcf91faa6d525a89b3a150e16d0dc884e6d95ea1d87168a248fcdd45e35121634` |
| 209 | 25928290 | 622 | transfer | 1902364797603184751334697 | 20 | baixa | movimento comum | `0xc2c621818922548a6cd1c4599b5187f0a5b932d46100a108ca0dbc83958abac2` |
| 210 | 25931034 | 704 | transfer | 1902364797603184751334697 | 20 | baixa | movimento comum | `0xc70e897f3a4d0bf5f641ce86612c172386d0756b07a004c91fb743ccfc013f3b` |
| 211 | 25941067 | 148 | transfer | 1261250370905561918372975 | 20 | baixa | movimento comum | `0x347687710d0b443e774d1e7ebd8d5c7503597f43cd11dd64deac11c022fd0efa` |
| 212 | 25941071 | 314 | transfer | 1261250370905561918372975 | 20 | baixa | movimento comum | `0x28e467e97f5993e6f828065d930e138fa0bc13ef45c52e828ed225e850daa262` |
| 213 | 25941791 | 751 | transfer | 1261250370905561918372975 | 20 | baixa | movimento comum | `0x933d37b53ae523082409a9169717d7e7ea41178cdb9763911e56cdd60c600eb5` |
| 214 | 25942136 | 766 | transfer | 8000000000000000000000000 | 44 | média | movimento relevante frente à oferta do bloco | `0x2094d71e08d0ba3847362b6fe16e7deed9604734c6761418f0c900c851aad615` |
| 215 | 25954868 | 176 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0xe2f10e5143c9710262b0de93a431885c76aec82d5b46062f4c2be337baf1afaa` |
| 216 | 25954868 | 177 | burn | 2999943000000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0xe2f10e5143c9710262b0de93a431885c76aec82d5b46062f4c2be337baf1afaa` |
| 217 | 26041724 | 664 | transfer | 2612485990000000000000000 | 20 | baixa | movimento comum | `0x38697f741e6b23da5485bdd52d30cd8c238484c15c0daa637ee196bac75266fd` |
| 218 | 26041773 | 518 | approval | 115792089237316195423570985008687907853269984665640564039457584007913129639935 | 56 | média | autorização sem limite; conferir uso e contexto | `0x009d5bd916c5684b0dd602f0f76ea9eb4e29a3b2fc62de015451324222fedf4a` |
| 219 | 26041774 | 729 | transfer | 2612485990000000000000000 | 20 | baixa | movimento comum | `0xb6e8801e1bfd848952da1470df92b65d85e577041794693cd6918171144f9ec6` |
| 220 | 26043542 | 421 | transfer | 300000000000000000000000 | 20 | baixa | movimento comum | `0x2af9abf9a9896019d0a142f04ab250bd53ba1f4b628eea33465e75767ccf4cea` |
| 221 | 26044373 | 157 | transfer | 300000000000000000000000 | 20 | baixa | movimento comum | `0xaf623930615b82d3e1a9579466b24d0f12eea97e95559840862f959de377876b` |
| 222 | 26054068 | 818 | transfer | 100000000000000000000000 | 20 | baixa | movimento comum | `0xd0152fe9f8db98d93011cdba09ad62bf7fbc7459614800716c7e286c049ea762` |
| 223 | 26054146 | 1040 | transfer | 100000000000000000000000 | 20 | baixa | movimento comum | `0x5894f5e16f81e6a6f8f293c89e1a192eeb4d20fdf2ee32d73ef56279059a2a27` |
| 224 | 26054605 | 787 | transfer | 204486490000000000000000 | 20 | baixa | movimento comum | `0x491f1a762f07e37907b2bbcbadd25e053d1ea59fb4d128b95f885207a08b828b` |
| 225 | 26054728 | 1210 | transfer | 100000000000000000000000 | 20 | baixa | movimento comum | `0x242df9fc9388adf10eb2bbd5e898b770a6c7662b04c4971b3e0428a6107d383f` |
| 226 | 26057837 | 1604 | transfer | 300000000000000000000000 | 20 | baixa | movimento comum | `0x3c8b9033bee2ed16ae4bb8e89a7fa042cf6fbc3700eb693b788f2aa1e9e21f1a` |
| 227 | 26091202 | 669 | mint | 140000000000000000000000000 | 76 | alta | mudança de oferta; conferir papel e balanço | `0x3774326cc60d1c91b5b2207bd03cad9d35c82d0bdb69ef56026d948271acaf4d` |
| 228 | 26098211 | 967 | mint | 140000000000000000000000000 | 76 | alta | mudança de oferta; conferir papel e balanço | `0x0b754bab5de105b11cafeff96cec458d0e2a8d647635525bbbb5b81d7a1b5879` |
| 229 | 26100061 | 913 | approval | 3000000000000000000000000 | 30 | baixa | autorização finita | `0x98dc4ec23458ce38be185bed661d7cd2b5bf7f26c7a8792ffc63f319ac04ede8` |
| 230 | 26100127 | 560 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0x94d331c5a5a9c050c4763f7bd874bbf8047313158ff2157966938cf35cdf379b` |
| 231 | 26100127 | 561 | burn | 2999943000000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x94d331c5a5a9c050c4763f7bd874bbf8047313158ff2157966938cf35cdf379b` |
| 232 | 26105487 | 303 | mint | 49300000000000000000000000 | 76 | alta | mudança de oferta; conferir papel e balanço | `0xc820d57d97c92737b72868356a4249c4e3af553e4480e793b54c7a6aead9b26b` |
| 233 | 26127288 | 243 | transfer | 5000000000000000000000000 | 44 | média | movimento relevante frente à oferta do bloco | `0xb2255842198616568e569a7d638d3811912e75019204689f5c513bb11c451a8b` |
| 234 | 26141280 | 751 | approval | 175000000000000000000000 | 30 | baixa | autorização finita | `0xe9f3d6c045f948477a476ad228c9b55ca6ac176d48cf56dfd135a40321ecbc19` |
| 235 | 26141596 | 776 | transfer | 57000000000000000000 | 20 | baixa | movimento comum | `0x8034da5f50230aacc1dfeb53f10168d98b9ac18ad06049ca94437721121a4194` |
| 236 | 26141596 | 777 | burn | 174943000000000000000000 | 53 | média | mudança de oferta; conferir papel e balanço | `0x8034da5f50230aacc1dfeb53f10168d98b9ac18ad06049ca94437721121a4194` |

Contagem das faixas dos registros: alta=12, média=53, baixa=171. Esta é uma fila de revisão; não são 12 falhas altas.

## Limites da conclusão

- A descoberta de contas com papéis usa as contas vistas no histórico; `complete_account_discovery=false`. Não afirmar enumeração total.
- 38/213 ramos vêm da fonte recompilada e 3048/5377 PCs vêm do bytecode no bloco; os denominadores medem coisas diferentes.
- Securify2 e Manticore não eram aplicáveis a este runtime; Certora não foi acionado; Kontrol permanece pendente. Isso limita a revisão, sem invalidar os resultados das ferramentas concluídas.
- O vínculo de fonte tem ressalva em três posições imutáveis declaradas. A decisão Rust é por afirmação e não uma prova de todos os estados.
- Nenhuma operação foi enviada ao contrato na rede pública. Os arquivos detalhados do trabalho permanecem no VPS privado.
