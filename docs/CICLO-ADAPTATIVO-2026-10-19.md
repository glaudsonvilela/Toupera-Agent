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

## Etapa 2 — andar pelo mapa (2026-10-19)

O planejador Rust agora olha para as saídas de blocos já vistos que levam a blocos ainda não vistos. Ele dá preferência a opções próximas dessas saídas, às famílias de operação que renderam novos caminhos e aos valores vizinhos de uma escolha produtiva. O executor monta sequências curtas na cópia local, como autorizar antes de mover ou retirar, dar saldo antes de mover e conceder um papel antes de usá-lo. Cada sequência é desfeita ao terminar. Também há entradas para autorização sem limite e assinaturas com valores diferentes. Essas entradas podem terminar sem alterar o estado, mas ajudam a mapear onde o código toma decisões.

Trabalho integrado `f7c70e446f134e78`, Ethereum bloco `26228428`: 40 escolhas, 18 sequências curtas e 18 operações aplicadas apenas na cópia local. A leitura inicial tinha 2470/5377 PCs; o ciclo acrescentou 578 PCs, 50 ligações e 38 blocos, chegando a 3048/5377 PCs observados em conjunto. Houve 11 passagens de bloco já visto para bloco novo. O ciclo parou no limite de 40 escolhas; por isso não se deve interpretar o número como cobertura máxima. A conferência Rust registrou zero erros de ligação e o guardião marcou a afirmação limitada como `OBSERVADO`. Não surgiram diferenças nas verificações de estado executadas. A medição de ramificações da fonte recompilada permaneceu em 38/213; ela não foi recalculada por este ciclo do bytecode.

Ainda faltam contas, valores, assinaturas válidas, estados e sequências fora das opções atuais. As ligações do mapa estático que não aparecem em uma rodada exigem confirmação por outros estados. Nenhuma ausência de diferença nesta rodada prova que o contrato inteiro está correto.
