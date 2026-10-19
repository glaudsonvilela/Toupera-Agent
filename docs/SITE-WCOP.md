# Painel wCOP no VPS

O painel em https://toupeiraagente.tech/#painel aceita nesta etapa apenas o endereço wCOP `0x8a1D45e102e886510e891d2Ec656a708991e2D76` na rede Ethereum. O usuário digita o endereço e a chave do painel. O terminal mostra eventos da execução; o relatório final aparece abaixo dele.

A API Python de loopback está em `tools/site_jobs.py`, executada por `toupera-site-jobs.service` como usuário `toupera`. O Caddy protege apenas `/api/smart*` com HTTP Basic, encaminha para 127.0.0.1:8765 e exige a origem do site para iniciar um trabalho. O segredo fica em `private-output/site-jobs/owner-password.txt` no VPS e nunca é enviado ao Git. Há um trabalho ativo por vez; cada trabalho tem diretório 0700, arquivos 0600 e JSON bruto privado.

## Fluxo

1. O formulário cria `/srv/toupera-agent/smart.md` e uma cópia por trabalho.
2. O trabalhador confirma chain ID, fixa bloco e hash, lê o runtime com `eth_getCode` e a implementação do proxy no slot EIP-1967. Não envia transações.
3. O Rust `security-engine --bytecode` processa o runtime do endereço e da implementação; o resumo vai para `result_smart.md`.
4. O trabalhador procura fonte verificada no Sourcify ou Blockscout e executa as ferramentas compatíveis. Cada uma das dez ferramentas fica marcada como executada, pendente, incompatível ou indisponível, com motivo. O relatório consolidado vai para `end_smart_result.md` e uma cópia por trabalho.
5. A API autenticada retorna somente os eventos de progresso e o Markdown final. O frontend o exibe como texto, sem interpretar HTML do contrato.

O motor Rust atual reconstrói a estrutura do runtime e mostra incertezas. Os sinais Slither/Mythril são brutos e não são achados confirmados. Foundry, Echidna, Medusa, Halmos e Kontrol exigem propriedades e harness específicos; Securify2/Manticore são legados para este runtime; Certora precisa de credencial e usa serviço externo. O painel registra esses limites em vez de alegar uma execução inexistente. Não há submissão automática ao programa de recompensas nem chamada à IA.

O endereço foi fornecido pelo usuário. O programa e o escopo podem mudar; confira as regras atuais em https://hackerone.com/ripio/policy_scopes antes de qualquer comunicação de resultados. O trabalho público de pesquisa visto online não é tratado como evidência original.

## Operação

- Serviço: `systemctl status toupera-site-jobs.service`.
- Origem da interface: `homepage/index.html`, `homepage/app.js`, `homepage/style.css`.
- API privada: `GET /api/smart/latest`, `POST /api/smart`.
- Arquivos raiz são o último trabalho; histórico privado fica em `private-output/site-jobs/<id>/`.
- O acesso à API é restrito ao titular da chave. Trocar a senha exige novo hash no Caddyfile e recarga do Caddy.

A primeira execução do wCOP no VPS (2026-10-19, bloco 26227621) identificou o proxy e a implementação `0x6f6206fac28d9f60d221a63aae5fb5c3a9ea6652`. O Rust contou 33 selectors e 475 incertezas na implementação. A fonte foi verificada pelo Blockscout; o Slither produziu 114 sinais brutos no conjunto de fontes, mas apenas um registro informativo de pragma incluiu o arquivo principal. O Mythril produziu um sinal genérico sobre timestamp, ainda sem confirmação de falha. Nenhum resultado foi submetido a terceiros. A senha do painel foi rotacionada após a publicação inicial e o diretório `deploy/` foi removido da raiz pública.
A execução final pela API ficou no diretório privado `private-output/site-jobs/b0a86a00d5664e97/` e terminou com oito eventos e relatório acessível pelo frontend autenticado. O servidor detecta trabalhador encerrado ou trabalho sem progresso e marca o resultado como incompleto, liberando novo envio.
