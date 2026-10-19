# wCOP: segunda medição de caminhos no VPS

- Trabalho: `2d97ca57278541bc`.
- Contrato: `0x8a1d45e102e886510e891d2ec656a708991e2d76` na Ethereum, bloco `26227913`.
- Escopo: apenas este contrato na Ethereum e uma cópia local da fonte para medir caminhos.

## Identidade do código

- Fonte: 34 arquivos, Solidity `v0.8.27+commit.40a35a09`, EVM `cancun`.
- Comparação com runtime da implementação: **confere sem campos imutáveis**. As posições imutáveis são marcadas pelo próprio compilador e foram excluídas da comparação.
- Entradas públicas: 33 na ABI; 33 seletores no Rust; 33 com ao menos um cenário local concluído.
- Isso mede entrada em funções, não todos os estados ou ramos de cada função.
- Espaços de dados nomeados na fonte: 6; posições distintas: sim. O layout comum do compilador não enumera os espaços ERC-7201 usados pelas bibliotecas.

## Execuções

- Foundry no bloco: 18/18 cenários, incluindo operações autorizadas, limites, assinatura, papéis e sequências variáveis.
- Foundry na fonte recompilada: 14 cenários.
- Echidna: executado; Medusa: executado. Duas regras locais comparam oferta e saldos em sequências.
- Halmos: concluído; 3 propriedades locais concluídas. As premissas desta prova local não cobrem todos os estados do contrato.
- Eventos lidos no intervalo: 236; oferta conferida com o bloco: sim.

## Comparação com a primeira medição

Cenários locais: 6 antes, 14 agora. Ramos observados no conjunto compilado: 19/213 antes, 38/213 agora. Isso mede caminhos percorridos na cópia local, não probabilidade de falha.

## Cobertura da fonte recompilada

| Área | Linhas | Funções | Ramos |
|---|---:|---:|---:|
| Arquivo próprio LatamStable.sol | 22/22 (100.0%) | 7/7 | 0/0 |
| Fonte e bibliotecas incluídas | 325/1126 (28.9%) | 96/280 | 38/213 (17.8%) |
| Arquivos com linhas executadas (18) | 325/493 | 96/145 | 38/77 |

A medição acima usa uma implantação local da mesma fonte. Ela não é cobertura direta do bytecode já publicado. A soma das bibliotecas inclui caminhos que podem não ser alcançáveis por este token; portanto, não é uma porcentagem do runtime. A seleção de arquivos executados depende desta rodada.

## Mapa das entradas públicas

| Função | Seletor | Cenários locais concluídos |
|---|---|---:|
| `DEFAULT_ADMIN_ROLE()` | `0xa217fddf` | 2 |
| `DOMAIN_SEPARATOR()` | `0x3644e515` | 1 |
| `MINTER_ROLE()` | `0xd5391393` | 1 |
| `PAUSER_ROLE()` | `0xe63ab1e9` | 2 |
| `UPGRADER_ROLE()` | `0xf72c0d8b` | 1 |
| `UPGRADE_INTERFACE_VERSION()` | `0xad3cb1cc` | 1 |
| `allowance(address,address)` | `0xdd62ed3e` | 2 |
| `approve(address,uint256)` | `0x095ea7b3` | 2 |
| `balanceOf(address)` | `0x70a08231` | 2 |
| `burn(uint256)` | `0x42966c68` | 1 |
| `burnFrom(address,uint256)` | `0x79cc6790` | 1 |
| `decimals()` | `0x313ce567` | 1 |
| `eip712Domain()` | `0x84b0196e` | 1 |
| `getRoleAdmin(bytes32)` | `0x248a9ca3` | 1 |
| `grantRole(bytes32,address)` | `0x2f2ff15d` | 1 |
| `hasRole(bytes32,address)` | `0x91d14854` | 1 |
| `initialize(address,address,address,address,string,string)` | `0x9317c200` | 1 |
| `mint(address,uint256)` | `0x40c10f19` | 2 |
| `name()` | `0x06fdde03` | 1 |
| `nonces(address)` | `0x7ecebe00` | 1 |
| `pause()` | `0x8456cb59` | 2 |
| `paused()` | `0x5c975abb` | 1 |
| `permit(address,address,uint256,uint256,uint8,bytes32,bytes32)` | `0xd505accf` | 1 |
| `proxiableUUID()` | `0x52d1902d` | 1 |
| `renounceRole(bytes32,address)` | `0x36568abe` | 1 |
| `revokeRole(bytes32,address)` | `0xd547741f` | 1 |
| `supportsInterface(bytes4)` | `0x01ffc9a7` | 1 |
| `symbol()` | `0x95d89b41` | 1 |
| `totalSupply()` | `0x18160ddd` | 2 |
| `transfer(address,uint256)` | `0xa9059cbb` | 2 |
| `transferFrom(address,address,uint256)` | `0x23b872dd` | 1 |
| `unpause()` | `0x3f4ba83a` | 1 |
| `upgradeToAndCall(address,bytes)` | `0x4f1ef286` | 1 |

## Lacunas observadas nos arquivos executados

- `@openzeppelin/contracts/utils/Address.sol`: 27 linhas e 9 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol`: 14 linhas e 8 ramos sem passagem.
- `@openzeppelin/contracts/utils/cryptography/ECDSA.sol`: 27 linhas e 7 ramos sem passagem.
- `@openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol`: 26 linhas e 7 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol`: 12 linhas e 3 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol`: 3 linhas e 3 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol`: 6 linhas e 1 ramo sem passagem.
- `@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol`: 3 linhas e 1 ramo sem passagem.
- `@openzeppelin/contracts/utils/StorageSlot.sol`: 16 linhas e 0 ramos sem passagem.
- `@openzeppelin/contracts/utils/cryptography/MessageHashUtils.sol`: 14 linhas e 0 ramos sem passagem.

## Arquivos incluídos com maiores lacunas

- `@openzeppelin/contracts/utils/math/SafeCast.sol`: 258 linhas e 64 ramos sem passagem.
- `@openzeppelin/contracts/utils/Strings.sol`: 148 linhas e 46 ramos sem passagem.
- `@openzeppelin/contracts/utils/math/Math.sol`: 211 linhas e 26 ramos sem passagem.
- `@openzeppelin/contracts/utils/Address.sol`: 27 linhas e 9 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/utils/cryptography/EIP712Upgradeable.sol`: 14 linhas e 8 ramos sem passagem.
- `@openzeppelin/contracts/utils/cryptography/ECDSA.sol`: 27 linhas e 7 ramos sem passagem.
- `@openzeppelin/contracts/proxy/ERC1967/ERC1967Utils.sol`: 26 linhas e 7 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol`: 12 linhas e 3 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol`: 3 linhas e 3 ramos sem passagem.
- `@openzeppelin/contracts-upgradeable/utils/NoncesUpgradeable.sol`: 6 linhas e 1 ramo sem passagem.

## Próximos passos verificáveis

1. Ampliar casos para ramos ainda não alcançados e classificar o que é alcançável pelo token.
2. Repetir sequências mais longas e registrar os caminhos percorridos no runtime publicado, além da fonte local.
3. Troca válida e estado foram medidos em cópia local; comparar versões futuras exige novas fontes.
4. Ampliar as três propriedades formais locais. Kontrol e Certora seguem sem prova deste caso.
5. Se o escopo for ampliado, relacionar chamadas dos contratos vizinhos; este documento não cobre outras redes.

**Estado:** 100% das entradas da ABI receberam ao menos um cenário local nesta rodada; a cobertura de ramos está longe de 100%. Nenhuma falha foi confirmada por esses números.
