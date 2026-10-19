# Ponto de retomada da análise forense wCOP — 2026-10-19

## Estado e identidade

- Trabalho principal: `92273047cbf7439c`, Ethereum bloco `26228846`. Trabalho de comparação: `7d78331aa23a4592`, bloco `26228635`.
- As verificações foram feitas no VPS, com o runtime e o bloco conferidos. As repetições de execução usaram uma cópia local no Anvil; não houve transação na rede principal.
- Nenhuma falha do contrato foi confirmada. O filtro anterior segue `local_review`, sem diferença local repetida.

## Reconciliação anterior

- Foram observados 3.014 de 5.377 PCs do runtime; 2.478 têm local atribuído pelo compilador. Os 536 PCs observados sem local têm `file_id = -1`. Nenhum dos 37 PCs posteriores ao mapa do compilador foi observado.
- As 74 ligações observadas fora do grafo estático saem de blocos terminados em `JUMP`: 68 com marca de saída de função e seis com marca de salto comum. Todas as 143 ligações observadas com origem terminada em `JUMPI` aparecem no grafo estático.
- A fonte recompilada teve 38/213 registros de ramo LCOV atingidos. Isso não mede os ramos do bytecode publicado. Ver `docs/WCOP-FORENSE-MAPA-2026-10-19.md`.

## Nova repetição por caso

- `tools/wcop_trace.py` e `tools/wcop_adaptive.py` agora permitem obter a pilha dos rastros locais sem alterar o comportamento padrão dos trabalhos. Os scripts `tools/wcop_baseline_branch_replay.py` e `tools/wcop_branch_replay.py` guardam somente condição, destino, próximo PC, contagem e local do compilador por caso; não guardam a pilha completa.
- A repetição dos 36 casos iniciais reproduziu exatamente o resumo salvo (casos, PCs e ligações). A repetição dos 38 casos escolhidos pelo Rust reproduziu os resultados salvos. Não houve desvio de próximo PC nem evento `JUMPI` incompleto.
- Foram executados também os 65 candidatos restantes do conjunto já gerado, isolados por snapshot no Anvil. Esses casos acrescentaram dois resultados de desvio ao conjunto observado, nos PCs `3439` e `6693`, sem acrescentar novo PC `JUMPI`. Ambos mapeiam para linhas de `ERC20Upgradeable.sol`; a linha por si só não identifica o ramo LCOV equivalente.
- O runtime contém 169 PCs `JUMPI` estáticos. No conjunto dos 36 + 38 + 65 casos, 111 PCs foram observados: 34 em ambos os sentidos, 77 em apenas um. Outros 58 PCs `JUMPI` não foram observados (36 com local de fonte, 22 de código gerado). Esses números não provam que os caminhos restantes sejam alcançáveis.
- Nenhuma divergência local foi reproduzida. Três preparações falharam entre os 38 casos já escolhidos pelo Rust, exatamente como na rodada salva; entre os 65 candidatos adicionais não houve falha de preparação. Rever essas três preparações antes de interpretar a ausência dos respectivos caminhos.

## Continuação dirigida no bloco fixado

- `tools/wcop_targeted_guards.py` executou 22 casos dirigidos de inicialização, UUPS, ERC-165, allowance, nonce e EIP-2612 no Anvil local. O conjunto passou de 111 para 155 dos 169 PCs `JUMPI` estáticos observados. Foram vistos 209 pares PC/sentido: 54 PCs nos dois sentidos e 101 em apenas um. Não houve próximo PC incompatível nem falha confirmada.
- `permit` com assinatura válida de conta de desenvolvimento concluiu em snapshot local: nonce +1 e allowance 1. A mesma assinatura falhou na repetição, sem alterar esses valores; o snapshot foi revertido. Chamadas autorizadas e não autorizadas de upgrade produziram os resultados esperados apenas nos casos medidos.
- Restam 14 PCs `JUMPI` não observados: seis com local de fonte e oito de Yul gerado. A maioria está ligada a alternativas de inicialização ou armazenamento de strings; isso não prova inalcançabilidade global. Ver `docs/WCOP-DESVIOS-DIRIGIDOS-2026-10-19.md`.

## Triagem dos guardas residuais

- `tools/wcop_residual_guards.py` executou 14 chamadas adicionais no fork local, incluindo endereços zero, papéis já concedidos/ausentes, `unpause` já despausado e dois alvos UUPS simulados somente no Anvil. As respostas foram as esperadas; 12 novos sentidos de `JUMPI` foram observados, sem divergência local ou falha confirmada.
- O conjunto atual observa 155/169 PCs `JUMPI`: 66 nos dois sentidos, 89 em um sentido e 14 ainda não observados; são 221 pares PC/sentido. `tools/wcop_residual_classify.py` separa os 89 em 9 do dispatcher, 31 entradas sem ETH, 25 do utilitário ABI gerado e 24 guardas de execução. A classificação é estrutural e não demonstra alcançabilidade dos lados restantes. Ver `docs/WCOP-GUARDAS-RESIDUAIS-2026-10-19.md`.

## Evidência privada

- `private-output/site-jobs/92273047cbf7439c/baseline-branch-replay.json`: 36 casos, SHA-256 `ff82321ad690aa3936433da544c73889a1ecb910f35395c72b651cd5b31aff8e`.
- `private-output/site-jobs/92273047cbf7439c/branch-replay.json`: 38 casos salvos + 65 adicionais, SHA-256 `06d09f85ab6a592db2b10e124ab5427e2572a802a3e4376a15ebcdcba618e135`.
- `private-output/site-jobs/92273047cbf7439c/targeted-guards.json`: 22 casos dirigidos, SHA-256 `47695f05e78e0e9317fa16450e446be26496b8a8fb23342bbe1c9403890b9611`.
- `private-output/site-jobs/92273047cbf7439c/residual-guards.json`: 14 casos, SHA-256 `97aeeaf34acd3c32f1f4f931f570668dc0b8f5e8f9815c63d8944386b745e941`.
- `private-output/site-jobs/92273047cbf7439c/residual-classification.json`: 89 desvios classificados, SHA-256 `870b834941cecf46de2dbd06fb50489bfa893c979e68353a230bee4aafbe9191`.
- Os JSONs acima permanecem privados, modo `0600`, propriedade de `toupera`. Este resumo não contém calldata, endereços de atores nem credenciais.

## Próxima ação

1. Examinar os 24 guardas de execução ainda vistos em um sentido, documentando pré-condição, alcance no proxy fixado e consequência de segurança; priorizar ECDSA e UUPS.
2. Examinar os 14 PCs ainda não observados em estados apropriados: inicialização fresca em ambiente local separado, strings longas e alternativas de EIP-712. Manter essas medidas separadas do proxy já inicializado e não supor alcançabilidade.
3. Vincular resultados de `JUMPI` à semântica da fonte, mantendo separados os IDs de ramo LCOV da recompilação. O estado histórico antigo ainda não está disponível no RPC (403).

## Publicação

O diretório `/srv/toupera-agent` no VPS não possui `.git` por desenho: a implantação usa `git archive` de um checkout autorizado, conforme `docs/VPS-SETUP.md`. A revisão instalada fica em `.deployed-commit`. O Git e as credenciais permanecem fora do VPS.
