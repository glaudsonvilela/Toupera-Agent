# Ciclo adaptativo local — 2026-10-19

## Fluxo

1. A leitura inicial fixa bloco, proxy, implementação e PCs observados.
2. O executor cria opções de operação, conta e valor a partir das funções conhecidas e do estado observado.
3. O Rust valida runtime e histórico, pontua opções pela cobertura ainda possível, pelo ganho anterior da família e por opções pouco exploradas.
4. A escolha roda em uma cópia Anvil local; PCs e transições novas alimentam a próxima escolha.
5. Mudanças em oferta, saldo, autorização, pausa e papel são comparadas com o efeito esperado da operação. Diferenças repetidas localmente são registradas para revisão.
6. O ciclo para por orçamento de 32 escolhas, por doze escolhas sem ganho ou por esgotamento das opções. Arquivos privados registram escolhas, cobertura e razões.

## Limites

Este é um planejador determinístico com feedback, não um modelo que aprende sem supervisão. A gramática cobre funções e contas selecionadas; valores e estados fora dela permanecem não examinados. O mapa estático ainda possui transições a explicar. Uma diferença local não confirma falha no contrato publicado. Somente a cópia local recebe transações.

## Rodada integrada

Trabalho integrado `fe5aa294573f4571` no VPS, Ethereum bloco `26228348`. A leitura inicial observou 2470/5377 PCs. O ciclo fez 18 escolhas, 7 operações ficaram registradas somente na cópia local e 11 chamadas retornaram sem aplicar estado. Foram observados 34 PCs e 2 transições novos além da leitura inicial, chegando a 2504/5377 PCs. O planejador parou após doze escolhas seguidas sem ganho. Não houve diferença repetível nas verificações de estado feitas; isso não exclui diferenças em caminhos não examinados. A reconciliação Rust registrou zero erros de ligação, e o guardião classificou a afirmação do ciclo como `OBSERVADO`, restrita a esse bloco e a essas escolhas.

A primeira rodada revelou que o recibo da cópia local pode chegar após a resposta de envio. O executor agora aguarda esse recibo por prazo limitado antes de decidir se a operação foi aplicada. Valores grandes são mantidos como inteiros decimais exatos ao passar pelo planejador Rust. O trabalho acima foi repetido depois dessas correções.
