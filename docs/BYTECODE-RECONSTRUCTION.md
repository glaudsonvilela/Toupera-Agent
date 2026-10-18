# Reconstrução de bytecode — M3, primeira etapa

O motor aceita **bytecode de runtime EVM legado** em hexadecimal, fornecido explicitamente em um arquivo local. Não consulta redes nem contratos por endereço.

```sh
cargo run -p security-engine -- --bytecode runtime.hex > runtime-ir.json
```

A biblioteca expõe `reconstruct_bytecode_hex(artifact, hex)`. O resultado `BytecodeAnalysis` mantém o identificador do artefato, tamanho original, bytes de código normalizados, instruções, blocos, arestas, selectors, efeitos e incertezas. Cada evidência de bytecode usa o PC original do runtime.

## Recuperado agora

- `0x` e espaços ASCII opcionais são removidos; bytes hex inválidos, entrada vazia e `PUSH` truncado são tratados explicitamente.
- Um sufixo de metadados Solidity só é removido se os dois últimos bytes indicarem um tamanho válido **e** o conteúdo for um mapa CBOR completo com chave conhecida (`solc`, `ipfs`, `bzzr0` ou `bzzr1`). Caso contrário, os bytes são preservados.
- A decodificação respeita os bytes imediatos de `PUSH`, inclusive `PUSH0`, de modo que um `JUMPDEST` dentro de um literal não vira destino. Sem a hard fork da rede como entrada, opcodes introduzidos em forks recentes recebem incerteza de disponibilidade.
- Blocos são separados no PC zero, em `JUMPDEST` e após instruções terminadoras. `JUMP` e `JUMPI` só recebem destino quando o valor vem de um `PUSH` imediatamente anterior e aponta para `JUMPDEST` válido. Saltos dinâmicos ou inválidos ficam em `unresolved`. `JUMPI` também conserva a aresta de queda.
- O padrão direto `PUSH4 selector → EQ → PUSH destino → JUMPI` recupera candidatos de dispatch. Ele recebe `Inferred` apenas quando há `DUP1` imediatamente antes e o bloco é alcançável pelo CFG conhecido; os demais candidatos ficam `Tentative`. Outros dispatches, inclusive binários com otimizador ou assembly diferente, podem não ser reconhecidos.
- `SLOAD`, `SSTORE`, `TLOAD`, `TSTORE`, chamadas EVM, criação e autodestruição são observados por opcode. Um slot literal só é associado quando um `PUSH` imediatamente anterior o fornece, com confiança `Inferred`. Não são inferidos alvos de chamada, nomes de funções nem layout completo de storage.
- Os efeitos alcançáveis por cada selector recuperado são reunidos em `entry_summaries` usando somente arestas conhecidas do grafo. Blocos não alcançáveis por essas arestas mantêm efeitos `Tentative`, pois podem ser dados ou destinos de saltos ainda não resolvidos. A marca `incomplete` aponta incerteza em um caminho alcançado.

## Comparação com source compilado localmente

```sh
cargo run -p security-engine -- --compare-bytecode runtime.hex solc-output.json Vault.sol Vault
```

`solc-output.json` precisa incluir `evm.deployedBytecode.object` e `evm.methodIdentifiers` do contrato escolhido. A função `compare_with_solc_contract` compara os bytes de runtime **após** a normalização de metadados e associa signatures somente a selectors recuperados. Quando o runtime não coincide ou não pode ser comparado, matches de método são `Tentative`. Método não reconhecido no dispatch não prova ausência da função; selector sem correspondência não prova código malicioso. Configurações do compilador, bibliotecas e artefato correto precisam ser estabelecidos antes de interpretar a comparação.

## Limites e próximos passos

Este recorte não executa EVM, não reconstrói stack/memória, não prova factibilidade de caminho e não segue fluxo de valor entre oracle, shares e assets. Saltos não literais e funções sem o padrão de dispatch reconhecido permanecem incertos. Bytecode de criação, containers EOF, placeholders de biblioteca e opcodes desconhecidos não recebem semântica inventada. O M3 continua aberto para propagação conservadora de stack, caminhos econômicos e comparação source/bytecode mais ampla.
