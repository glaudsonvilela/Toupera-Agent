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

## Evidência privada

- `private-output/site-jobs/92273047cbf7439c/baseline-branch-replay.json`: 36 casos, SHA-256 `ff82321ad690aa3936433da544c73889a1ecb910f35395c72b651cd5b31aff8e`.
- `private-output/site-jobs/92273047cbf7439c/branch-replay.json`: 38 casos salvos + 65 adicionais, SHA-256 `06d09f85ab6a592db2b10e124ab5427e2572a802a3e4376a15ebcdcba618e135`.
- Os JSONs acima permanecem privados, modo `0600`, propriedade de `toupera`. Este resumo não contém calldata, endereços de atores nem credenciais.

## Próxima ação

1. Priorizar os 58 PCs `JUMPI` não observados por função e pré-condição. Um deles está em `src/LatamStable.sol`, PC `1186`, linha `23`; 22 pertencem a código gerado pelo compilador. Separar código de biblioteca potencialmente inalcançável de caminhos cuja pré-condição pode ser construída localmente.
2. Criar casos dirigidos apenas para caminhos com pré-condições verificáveis, registrar o resultado por caso e repetir qualquer divergência sob a mesma identidade de bloco/runtime. Não interpretar ausência de cobertura como prova de segurança ou de vulnerabilidade.
3. Vincular o resultado de cada `JUMPI` à semântica da fonte, mantendo separados os IDs de ramo LCOV da recompilação. O estado histórico antigo ainda não está disponível no RPC (403).

## Publicação

O diretório `/srv/toupera-agent` no VPS não possui `.git` por desenho: a implantação usa `git archive` de um checkout autorizado, conforme `docs/VPS-SETUP.md`. A revisão instalada fica em `.deployed-commit`. O Git e as credenciais permanecem fora do VPS.
