# Reconstrução de bytecode — M3, recorte legado concluído

O motor aceita **bytecode de runtime EVM legado** em hexadecimal, fornecido explicitamente em arquivo local. Não consulta redes nem contratos por endereço.

```sh
cargo run -p security-engine -- --bytecode runtime.hex > runtime-ir.json
```

A API `reconstruct_bytecode_hex(artifact, hex)` retorna `BytecodeAnalysis` com bytes normalizados, instruções, blocos, arestas, selectors, efeitos, dependências de valor e incertezas. Os PCs se referem ao runtime original, antes de remover o sufixo de metadados.

## Segunda leitura local com EVMole

O mesmo comando também executa [EVMole 0.9.4](https://github.com/cdump/evmole) como biblioteca Rust **local**, sobre os bytes de runtime já normalizados. A saída `evmole` é separada dos fatos nativos e registra versão, selectors ABI/fallback, possíveis tipos de argumentos, mutabilidade, layout de storage persistente/transitório, arestas de CFG e incertezas. Cada selector tem PC de entrada; cada aresta tem PC de origem e destino. O layout de storage do EVMole não fornece PC para toda leitura/escrita: `corroborating_access_pcs` contém apenas acessos que o decoder nativo observou com o mesmo slot literal e selector alcançável. Lista vazia significa ausência dessa confirmação, não ausência de acesso.

`matches_native` mostra acordo entre analisadores. Divergência de destino ou selector exclusivo de um deles fica explícita em `evmole.unresolved`. O EVMole nunca altera automaticamente o CFG, os efeitos ou o fluxo de valor nativos; seus tipos, storage e arestas continuam `Tentative`. Selector candidato só entra no relatório se apontar para `JUMPDEST` decodificado. Para limitar custo, o companion é omitido acima de 24.576 bytes e registra o motivo; um erro interno do EVMole também preserva a análise nativa.

## Leituras opcionais com Heimdall e Gigahorse

O comando `--bytecode-companions` importa dois artefatos **locais** produzidos pelas ferramentas abertas [Heimdall-rs](https://github.com/Jon-Becker/heimdall-rs) e [Gigahorse](https://github.com/nevillegrech/gigahorse-toolchain):

```sh
security-engine --bytecode-companions runtime.hex heimdall/abi.json gigahorse/out/contract.tac > combined.json
```

`native` contém a análise Rust/EVMole existente. `heimdall.functions` preserva selector, tipos de entrada sugeridos, mutabilidade e PCs de entrada corroborados pelo decoder nativo e/ou EVMole. A ABI padrão do Heimdall 0.9.x não inclui um campo `selector`; o importador recupera o seletor do nome sintético `Unresolved_0x...` gerado com `--skip-resolving`. Um campo `selector` explícito também é aceito em formatos futuros. Assinatura, tipos e mutabilidade são sempre `Tentative`; `--skip-resolving` evita consulta de assinaturas a um serviço externo. Funções sem selector válido são omitidas com incerteza. `gigahorse.blocks` importa labels de bloco e sucessores do TAC textual, aceita apenas PCs de instrução já decodificados e indica se o início coincide com um bloco nativo. Essas arestas também são `Tentative`; expressões TAC não viram fluxos de valor nativos. Saídas inválidas geram erro de importação ou incerteza explícita.

Em uma máquina com Heimdall, Podman e a imagem oficial do Gigahorse instalados, o wrapper local executa as duas ferramentas e guarda os artefatos:

```sh
TOUPERA_SECURITY_ENGINE=/srv/toupera-agent/target/release/security-engine \
  scripts/run-bytecode-companions.sh runtime.hex /tmp/contract-companions
```

O diretório de saída deve ser novo. Ele contém `runtime.hex`, `heimdall/abi.json`, `gigahorse-temp/runtime/out/contract.tac`, `analysis.json`, a versão do Heimdall e o digest da imagem Gigahorse. O wrapper passa apenas bytecode em arquivo, desliga a rede do contêiner Gigahorse, usa `--skip-resolving` no Heimdall e não ativa pós-processamento por LLM. Há limites de 5 minutos para Heimdall, 20 minutos para Gigahorse e 16 MiB para importação do TAC. A execução é opcional; `--bytecode` continua funcionando sem as ferramentas. O wrapper falha explicitamente se algum artefato esperado faltar. A imagem Gigahorse padrão é fixada por digest; `TOUPERA_GIGAHORSE_IMAGE` permite substituir por outra versão instalada.

## Recuperação estrutural

- Hex aceita `0x` e espaços ASCII. Entradas inválidas são rejeitadas; `PUSH` truncado é registrado como incerteza.
- Metadados Solidity só são removidos quando o sufixo tem tamanho válido, mapa CBOR completo e chave conhecida (`solc`, `ipfs`, `bzzr0`, `bzzr1`). Outros bytes são preservados.
- O decoder respeita os imediatos de `PUSH`, inclusive `PUSH0`. Um `JUMPDEST` embutido em dado não vira destino. Sem hard fork informado, opcodes recentes recebem incerteza de disponibilidade.
- Blocos começam em PC zero, `JUMPDEST` ou após terminador. `JUMP` e `JUMPI` ligam apenas destinos constantes que apontam para `JUMPDEST` decodificado. A constante pode vir de `PUSH`, operações inteiras simples ou propagação de stack/memória por palavras. Destinos ainda dinâmicos ou inválidos ficam em `unresolved`. `JUMPI` conserva também a queda; o grafo não prova que nenhum ramo é factível.
- O padrão direto `PUSH4 selector → EQ → PUSH destino → JUMPI` produz candidato de dispatch. `Inferred` exige bloco alcançável, `DUP1` anterior e `CALLDATALOAD`/`SHR` próximos; os demais candidatos são `Tentative`. Dispatch binário de otimizador, assembly e outros padrões podem não ser reconhecidos.
- `SLOAD`, `SSTORE`, `TLOAD`, `TSTORE`, chamadas, criação e autodestruição são observados por opcode e PC. Slot literal só é associado quando um `PUSH` imediatamente anterior fornece a chave. Os efeitos alcançáveis por selector são reunidos em `entry_summaries` por arestas conhecidas.

## Dependências de valor

O intérprete abstrato acompanha uma stack EVM e palavras de memória de 32 bytes por bloco. Ele propaga constantes pequenas, origens e PCs de aritmética por arestas conhecidas até um ponto fixo limitado. `BytecodeValueFlow` liga origens observáveis (`Calldata`, `CallValue`, `StorageRead`, `ExternalReturn`, sucesso de chamada externa e `Balance`) a escrita de storage, destino/valor/entrada de chamada, retorno ou condição. O fluxo do retorno de `CALL`/`STATICCALL` é recuperado quando offset e tamanho de saída conhecidos permitem acompanhar a primeira palavra. O fluxo de `RETURNDATACOPY` conhecido também é acompanhado. Cada relação inclui PC da origem, do sink e da aritmética contribuinte, além de confiança.

Essas relações mostram **dependência possível**, não valor concreto, identidade de oracle, asset, share ou factibilidade. Memória parcialmente conhecida, saídas de chamadas externas, junções de caminhos e resumos compartilhados por selectors reduzem confiança. Um resumo por selector classifica seu fluxo como `Tentative`, porque o mesmo bloco pode ser alcançado por outros selectors. Stack inconsistente, opcode sem semântica modelada, salto dinâmico, chave de storage não constante e limite de propagação ficam em `unresolved`; o motor não inventa arestas ou fluxos para preencher lacunas.

## Comparação com source compilado localmente

```sh
cargo run -p security-engine -- --compare-bytecode runtime.hex solc-output.json Vault.sol Vault
```

`solc-output.json` precisa conter `evm.deployedBytecode.object` e `evm.methodIdentifiers`. A comparação exige bytes de runtime normalizados idênticos antes de atribuir rótulos econômicos baseados no nome de métodos (`deposit`, `mint`, `withdraw`, `redeem`, previews, conversões e totais). Se o compilador fornecer `immutableReferences`, a comparação também pode aceitar igualdade **fora exclusivamente dessas faixas**, guardando os valores observados no runtime e mantendo `normalized_runtime_match=false` e `immutable_masked_runtime_match=true` como fatos distintos. Referências inválidas ou divergências fora das faixas não passam. `matched_methods.recovered_by` distingue `native`, `evmole` e `native+evmole`; um selector ABI recuperado apenas pelo EVMole continua `Tentative`, mesmo com source correspondente. Divergência de destino entre os dois também força `Tentative`. Esses rótulos vêm do método compilado, não de uma inferência semântica de quatro bytes do selector. Se runtime divergir ou não puder ser comparado, matches de método não recebem rótulo. Configurações do compilador e links de biblioteca precisam corresponder ao alvo para a igualdade ser significativa. O [piloto Sepolia](REAL-EVM-PILOT.md) observou duas faixas `immutable` no contrato real.

## Limites do recorte

M3 está concluído para essa reconstrução conservadora de **runtime legado explícito**. A segunda leitura acrescenta hipóteses de ABI e storage; ela não fornece ABI ou layout comprovados, decompilação completa, execução simbólica ou detecção de vulnerabilidade. Bytecode de criação, EOF, placeholders de biblioteca, saltos não resolvidos, identidade de alvos externos, estruturas de dados em memória e interpretação econômica de bytecode isolado ficam fora do recorte. M4 terá detectores econômicos determinísticos; um achado só avança depois de reprodução local e revisão.

Referências da semântica usada: [tabela de opcodes da Ethereum](https://ethereum.org/developers/docs/evm/opcodes/) e [metadados Solidity](https://docs.soliditylang.org/en/latest/metadata.html).
