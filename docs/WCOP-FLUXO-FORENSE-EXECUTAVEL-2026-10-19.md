# Fluxo forense executável do wCOP — 2026-10-19

## Alvo e desenho

O fluxo opera sobre o trabalho privado `92273047cbf7439c`, proxy wCOP no bloco Ethereum `26228846`. Todos os ensaios de estado usam um fork local Anvil com identidade do bytecode conferida; não enviam transações à rede principal. O código e a documentação ficam no repositório, enquanto os JSONs detalhados permanecem em `private-output` no VPS.

`tools/wcop_forensic_run.py 92273047cbf7439c --refresh` executa, em ordem e com tempo limitado, os ensaios residuais, a revisão clínica, a classificação de PCs, a reconciliação histórica e as sequências de invariantes. Depois chama o motor Rust com `wcop-forensic-sift <job-dir>`. Sem `--refresh`, o comando apenas reavalia os relatórios existentes. O resultado é salvo como `forensic-sift.json` privado, modo `0600`.

## Reconciliação histórica

`tools/wcop_historical_reconcile.py` compara as transações relevantes do histórico salvo com detalhes e logs transacionais obtidos do Blockscout. Verifica hash, bloco, status, índice do log, endereço do proxy, tópico e argumentos indexados de upgrade e papéis.

| Evidência | Resultado |
| --- | ---: |
| Transações relevantes conferidas | 5 |
| Eventos conferidos | 17 |
| Upgrade / inicialização | 1 / 1 |
| Concessões / retiradas de papel | 11 / 4 |
| Transações sem status `ok` no conjunto | 0 |

Os logs corroboram a emissão dos eventos e a sequência de papéis. O RPC de arquivo para ler armazenamento em blocos antigos continua indisponível; por isso, o relatório registra `historical_storage_status=not_verified_by_archive_rpc`. Não se infere um valor de slot histórico apenas dos eventos. A conferência atual de papéis já estava reconciliada no trabalho salvo.

## Invariantes no fork

`tools/wcop_invariant_sequences.py` abre um fork fixado e isola cada cenário por snapshot, conferindo restauração do estado depois. Os sete cenários são emissão não autorizada, emissão autorizada, conservação em transferência, conservação em queima, pausa→transferência recusada→retomada, aprovação→`transferFrom` e concessão→emissão→retirada de `MINTER_ROLE`.

Os resultados observaram as reversões esperadas, variações de `totalSupply` e saldos de uma unidade quando apropriado, consumo de allowance, bloqueio de transferência durante pausa e bloqueio de emissão depois da retirada do papel. O slot de implementação permaneceu igual nesses cenários. Os sete snapshots foram restaurados. São testes de contas, valor e sequência específicos, não prova para todos os estados.

## Filtro Rust e decisão

`wcop_forensic_sift.rs` lê os relatórios privados como arquivos regulares com limites de tamanho, valida bloco, contrato, runtime, identificador do trabalho e hashes das quatro repetições de `JUMPI`. Recompõe os PCs observados, os 14 não vistos e os 23 guardas de execução ainda vistos em um único sentido. Organiza estes últimos em sete grupos com pré-condição e próximo exame: ECDSA (7 PCs), UUPS (3), retorno de delegatecall (2), código da implementação (1), inicialização (4), armazenamento EIP-712 (4) e nome ERC-20 (2).

A decisão atual é `hold_no_confirmed_violation`, `candidate_count=0` e `ready_for_report=false`. A regra do filtro exige uma violação de estado ou autorização repetível e com consequência antes de promover um candidato. Cobertura incompleta, reversões esperadas e dados de outro contexto não passam por esse gate. O filtro não faz consulta externa nem envia transação. Uma cópia com bloco adulterado em `residual-guards.json` foi rejeitada, comprovando a checagem de identidade.

| Artefato privado | SHA-256 |
| --- | --- |
| `historical-reconciliation.json` | `6a279d1d5f3f682c675c0ec247c0ed43c0cacb6180a205c55353edd1313159a9` |
| `invariant-sequences.json` | `bc0c58827ab7a1e96aca4b962f8ee44ae76df9655448f68d270f1d3b887eb669` |
| `forensic-sift.json` | `f168b0537c529c9601d584f846687b088b026da4e6a2677f49237d5a108a78b0` |

O motor compilou em modo release; `py_compile` passou nos scripts. O serviço `toupera-site-jobs.service` permaneceu ativo. O diretório `/srv/toupera-agent` não usa `.git`; a sincronização continua por `git archive` do checkout autorizado externo.

## Lacunas preservadas

O armazenamento histórico no bloco de implantação ainda não foi verificado por RPC de arquivo. Os 14 PCs nunca observados e os guardas de nome, versão e inicialização exigem estados locais preparados e medição separada do proxy implantado. Nenhuma vulnerabilidade foi confirmada; não há base atual para relato à Ripio.
