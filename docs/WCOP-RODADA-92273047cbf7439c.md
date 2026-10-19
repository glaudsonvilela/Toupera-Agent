# Rodada integrada após ajuste do mapa — 92273047cbf7439c

- Trabalho concluído no VPS em 2026-10-19, Ethereum bloco `26228846`, mesmo runtime da rodada anterior. O processo gerou `smart.md`, `result_smart.md` e `end_smart_result.md` no diretório privado do trabalho.
- SHA-256 dos três arquivos, na mesma ordem: `4754e0a3f1c06f8fc62f3f377b9359d0ac60499cc23fb36530f549f27949ad7b`, `483b3ef062fd3631978d9823c40e34aa8e914b2991142de4dd77828d8d85b4f0`, `86bf1eca3be1092d48a90f031857e6b84d05bb31b514efc3628d7a4632956c04`.

## Medidas

- O ciclo local fez 38 escolhas e parou por platô: 544 PCs novos e 48 ligações novas. Os 544 PCs ficaram dentro do intervalo do mapa do compilador; zero ficaram depois do PC 8499.
- Total observado: 3014/5377 posições do runtime. Dessas, 2478 receberam local de fonte ou arquivo interno do compilador. As 37 posições após o mapa permanecem sem observação.
- O mapa registrou 74 ligações observadas fora das ligações estáticas. A fonte recompilada mediu 38/213 ramos; esta é outra medida e o ciclo do runtime não atualiza essa contagem.
- O vínculo de identidade foi aceito. O filtro final ficou em `local_review`, com zero itens liberados para contexto externo e nenhuma diferença local repetida. A linha de versões ficou limitada porque o estado de um bloco antigo não estava disponível.
- Foundry registrou 18 cenários locais concluídos. Echidna e Medusa executaram duas regras locais cada; Halmos concluiu três propriedades limitadas. As ferramentas não aplicáveis, indisponíveis ou pendentes continuam indicadas no trabalho privado.

## Comparação e limite

- A rodada anterior, `7d78331aa23a4592`, usou o bloco `26228635` e também observou 3014 posições no total e 544 PCs novos no ciclo. A igualdade dessas contagens entre blocos distintos não demonstra cobertura completa nem melhora de descoberta.
- O ajuste comprovou a separação das posições após o mapa no orçamento de escolha e nas contagens do executor. Não houve resultado confirmado sobre o contrato.
- Próxima etapa: classificar as posições sem linha dentro do intervalo do mapa e ligar as novas passagens do runtime à medição de ramos da fonte.
