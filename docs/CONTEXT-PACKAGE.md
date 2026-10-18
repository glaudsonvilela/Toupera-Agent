# M5 — deduplicação e pacote de contexto

O CLI constrói um pacote local por arquivo Solidity explícito. Ele executa o `solc` local, reconstrói a IR do M2, aplica a triagem M4 e produz um slice por candidato deduplicado. O recorte continua limitado a um arquivo sem imports. Candidatos são `UNVERIFIED`; o pacote não confirma vulnerabilidades nem chama uma IA.

```sh
umask 077
mkdir -m 700 -p private-output
./target/release/security-engine --context-package Vault.sol private-output/context-cache /usr/local/bin/solc > private-output/context.json
./target/release/security-engine --context-package Vault.sol private-output/context-cache /usr/local/bin/solc --slither slither.json --bytecode runtime.hex > private-output/context.json
```

`solc-path` deve apontar para o arquivo executável do compilador, pois seus bytes entram na chave do cache. `--slither` aceita o JSON local opcional do M4; `--bytecode` aceita um runtime hex legado opcional. O runtime é normalizado, com remoção de metadados Solidity reconhecidos, e **somente identificado por hash**. Não se presume que corresponda ao source fornecido. Arquivos de saída podem conter trechos de código privado; use `umask 077`, diretórios privados e retenção controlada.

## Chaves e reuso

- SHA-256 do source exato, do executável `solc`, do binário do motor, da AST compilada e de cada função com corpo. Quando informado, registra também SHA-256 do JSON Slither e dos bytes do runtime normalizado.
- A chave do cache combina versão do formato, nome do arquivo, source, compilador, motor, Slither e runtime. Um acerto válido pula `solc`, reconstrução da IR, triagem M4 e montagem dos slices. Mudança no motor ou em qualquer entrada invalida esse resultado.
- O cache é local, em diretório modo `0700`, com entradas regulares modo `0600`; o CLI rejeita entradas públicas. `cache_hit` distingue reuso de construção. O cache armazena apenas o pacote reduzido, não o source completo, o JSON bruto do Slither nem a resposta do compilador.

## Conteúdo de cada slice

O slice contém o candidato M4, função com trecho de source limitado a 4 KiB, até três funções relacionadas com até 1,5 KiB cada, até doze trechos de evidência com até 384 bytes cada, listas limitadas de storage lido/escrito, aritmética, alvos de oracle e chamadas resolvidas, além de perguntas de invariantes da categoria e incertezas. `truncated` sinaliza cortes. `function_digests` identifica funções no arquivo, mas não inclui seus corpos. `economic_metrics` expõe o volume de sinais e candidatos do M4.

O M6 seleciona apenas slices com evidência e perguntas não resolvidas, e prepara um plano local ou uma chamada DeepSeek autorizada explicitamente. O comando `--context-package` em si não envia dados a um serviço externo. O cache economiza reconstruções idênticas; economia de tokens e qualidade de achados dependem de medição após a implementação completa, conforme pedido do usuário. Leia [Revisão com IA](LLM-REVIEW.md).
