# Isolamento do serviço e vínculo M8 — 2026-10-19

## Mudanças

1. O serviço do painel opera como `toupera` sob `ProtectSystem=strict`, `PrivateTmp`, sem capacidades adicionais e com escrita permitida somente em `private-output` e caches de ferramentas. O código e o binário passaram a pertencer ao `root` e são somente para leitura pelo serviço.
2. `smart.md`, `result_smart.md` e `end_smart_result.md` continuam na raiz como links para `private-output/root-files`. O escritor atômico foi ajustado para conservar os links.
3. O M8 aceita `--guard-job <job-dir>`. Antes de criar o pacote, o Rust recalcula a decisão do guardião, compara-a com o registro salvo e exige vínculo de fonte e hash da fonte principal iguais ao caso M7.
4. O pacote M8 inclui `guard/decision.json`, `guard/job-manifest.json` e `guard/compiler-input.json`, cada um com SHA-256 no manifesto. Na finalização, o Rust reconfere esses hashes, endereço, bloco, implementação, decisão e fonte, além dos critérios anteriores de reprodução e revisão humana.
5. Casos M8 sem um trabalho ligado permanecem permitidos, mas carregam uma limitação explícita. Um vínculo com campos imutáveis mascarados também é indicado como ressalva.
6. A execução Foundry no painel passa a repetir somente quando o provedor RPC devolve erro transitório 429/502/503/504; logs das tentativas anteriores ficam no trabalho privado.

## Conferências no VPS

- `cargo build --release -p security-engine`: concluído.
- Serviço reiniciado e ativo. `systemd-analyze security`: 4.8 OK; antes: 9.0 UNSAFE.
- Usuário `toupera`: sem escrita em `tools/site_jobs.py`; com escrita em `private-output`.
- Site público respondeu HTTP 200.
- Trabalho `8e6d50af37014b4d`: completou com identidade válida, fonte `masked_immutables`, três arquivos da raiz preservados. Uma chamada Foundry falhou por resposta HTTP 502 do RPC: 17/18 casos locais passaram; isso não indica diferença do contrato.
- Pacote de integração M8 privado: `PENDING_REPRODUCTION`, três arquivos `guard/` presentes; trabalho antigo com binário diferente foi recusado como decisão desatualizada.
- Trabalho posterior `d59cb58cee084b10`: completo no bloco 26228205, identidade válida, fonte `masked_immutables`; os 18/18 cenários Foundry locais passaram, sem necessidade de repetição. Os resultados permanecem no VPS privado.

## Limites

O VPS não oferece isolamento de namespaces de usuário para cada ferramenta. O serviço e seus processos filhos compartilham o mesmo usuário e alguns caches permitidos; a política do systemd e as permissões de arquivo reduzem o alcance de escrita, mas não são uma partição formal de confiança. O M8 ligado não prova que o contrato é seguro: exige reprodução local, revisão humana e premissas explícitas. A comparação com campos imutáveis mascarados não confere seus valores individualmente.
