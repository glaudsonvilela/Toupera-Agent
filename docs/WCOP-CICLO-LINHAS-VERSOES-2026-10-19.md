# wCOP — mapa, escolhas e versões (2026-10-19)

## Caminho implementado

1. A recompilação pede mapa de linhas e arquivos internos do compilador. A fonte só é ligada ao código publicado após conferir o runtime e a ressalva dos campos imutáveis.
2. O leitor local separa posições atribuídas à fonte, ao conteúdo gerado e a locais sem atribuição. Ele reúne as ligações observadas nas duas leituras.
3. O Rust confere identidade e ordena essas ligações. Por entrada pública, estima onde ainda existem posições de fonte não visitadas; o resultado orienta o próximo ciclo local.
4. O ciclo Rust usa o ganho observado e a ordem do mapa, com retorno menor para escolhas repetidas. O executor permite sequências curtas de até duas preparações e restaura a cópia local depois de cada caso.
5. A linha de versões consulta blocos fixos e o Rust confere os registros de troca com o código e a posição vistos. Se o provedor negar o estado histórico, a lacuna fica explícita; o código atual permanece conferido pelo trabalho.
6. O filtro Rust só permite preparar contexto adicional quando há diferença repetida localmente e os vínculos do trabalho estão conferidos. Esta etapa não faz chamada externa.

## Medição retrospectiva anterior

- Trabalho `7011fcd042db41e5`, bloco `26228465`.
- Antes dos arquivos internos, 1945/3048 posições observadas tinham local; depois, 2504/3048, das quais 559 são de conteúdo gerado.
- As duas leituras reuniram 74 ligações observadas fora do mapa estático. O Rust ordenou as 74 e calculou prioridades para 33 entradas.
- A linha de versões encontrou um registro de troca e dois blocos de interesse. O provedor recusou estado histórico antigo; a versão final foi conferida pelos arquivos guardados do trabalho concluído. Estado: `limited_historical_state`, uma mudança sem bloco histórico verificável.
- Nenhuma diferença de estado do contrato foi confirmada por essas medições.

## Limites

- A ordem dos caminhos é uma forma de gastar melhor o orçamento, não uma previsão de resultado.
- O total de posições desmontadas inclui trechos depois do mapa do compilador; não é denominador de caminhos alcançáveis.
- Arquivos internos do compilador não equivalem a linhas escritas pelos autores do contrato.
- Uma leitura por bloco não separa operações distintas ocorridas dentro da mesma transação ou do mesmo bloco.
- O ciclo tem limite de escolhas e de tempo. Se não aparecer diferença reproduzível, o filtro conserva a decisão local.

## Nova rodada integrada

Pendente de conclusão do trabalho iniciado após a integração.
