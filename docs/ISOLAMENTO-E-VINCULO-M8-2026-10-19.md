# Isolamento do serviço e vínculo M8 — 2026-10-19

## Mudanças

1. O serviço do painel opera como `toupera` sob `ProtectSystem=strict`, `PrivateTmp`, sem capacidades adicionais e com escrita permitida somente em `private-output` e caches de ferramentas. O código e o binário passaram a pertencer ao `root` e são somente para leitura pelo serviço.
2. `smart.md`, `result_smart.md` e `end_smart_result.md` continuam na raiz como links para `private-output/root-files`. O escritor atômico foi ajustado para conservar os links.
3. O M8 aceita `--guard-job <job-dir>`. Antes de criar o pacote, o Rust recalcula a decisão do guardião, compara-a com o registro salvo e exige vínculo de fonte e hash da fonte principal iguais ao caso M7.
4. O pacote M8 inclui `guard/decision.json`, `guard/job-manifest.json` e `guard/compiler-input.json`, cada um com SHA-256 no manifesto. Na finalização, o Rust reconfere esses hashes, endereço, bloco, implementação, decisão e fonte, além dos critérios anteriores de reprodução e revisão humana.
5. Casos M8 sem um trabalho ligado permanecem permitidos, mas carregam uma limitação explícita. Um vínculo com campos imutáveis mascarados também é indicado como ressalva.
6. A execução Foundry no painel passa a repetir somente quando o provedor RPC devolve erro transitório 429/502/503/504; logs das tentativas anteriores ficam no trabalho privado.

7. Cada ferramenta chamada por `command()` roda em `bubblewrap` com montagem, PID, IPC e host separados. O trabalho atual é o único visível sob `private-output`; o restante do sistema é somente para leitura. Mythril usa cópia privada de cache, e o processo local de caminhos passa pelo mesmo executor. O pacote `bubblewrap` é uma dependência do serviço.

## Conferências no VPS

- `cargo build --release -p security-engine`: concluído.
- Serviço reiniciado e ativo. `systemd-analyze security`: 4.8 OK; antes: 9.0 UNSAFE.
- Usuário `toupera`: sem escrita em `tools/site_jobs.py`; com escrita em `private-output`.
- Site público respondeu HTTP 200.
- Trabalho `8e6d50af37014b4d`: completou com identidade válida, fonte `masked_immutables`, três arquivos da raiz preservados. Uma chamada Foundry falhou por resposta HTTP 502 do RPC: 17/18 casos locais passaram; isso não indica diferença do contrato.
- Pacote de integração M8 privado: `PENDING_REPRODUCTION`, três arquivos `guard/` presentes; trabalho antigo com binário diferente foi recusado como decisão desatualizada.
- Trabalho posterior `d59cb58cee084b10`: completo no bloco 26228205, identidade válida, fonte `masked_immutables`; os 18/18 cenários Foundry locais passaram, sem necessidade de repetição. Os resultados permanecem no VPS privado.

- Trabalho `3fdf10704a64482c`, bloco 26228247: rodada completa sob isolamento por execução, guardião com identidade válida e `masked_immutables`; Slither, Mythril, Foundry (18/18), Echidna, Medusa e Halmos concluíram suas fatias; leitura local de caminhos e reconciliação disponíveis. Securify2/Manticore seguem não aplicáveis a este runtime; Certora não possui chave; Kontrol segue pendente de propriedade e premissas.
- A visão isolada foi conferida: escrita no trabalho atual permitida; outro trabalho oculto sob `private-output`. `--evidence-plan` ligado ao trabalho final criou pacote `PENDING_REPRODUCTION` com os três artefatos de guardião; nenhum resultado real M7 foi promovido.
- O endpoint público respondeu HTTP 200 após a mudança. Logs completos e caches privados permanecem no VPS.

## Limites

Bubblewrap cria namespaces por execução e oculta os outros trabalhos, mas todos os subprocessos ainda executam com o usuário `toupera` e compartilham a rede para leituras RPC. O processo coordenador do painel enxerga os trabalhos. A política reduz o alcance de leitura e escrita das ferramentas, mas não constitui uma partição formal de confiança. O M8 ligado não prova que o contrato é seguro: exige reprodução local, revisão humana e premissas explícitas. A comparação com campos imutáveis mascarados não confere seus valores individualmente.
