# Foco do mapa do runtime — 2026-10-19

## Alteração

- O Rust confere o último PC coberto pelo mapa do compilador contra as instruções e as duas leituras locais. Também confere as contagens de posições posteriores ao mapa. Ligações posteriores permanecem no relatório, mas não entram na prioridade por entrada.
- O executor passa esse limite ao planejador Rust. Ganhos em PCs, ligações e posições incertas até o limite orientam a próxima escolha, a detecção de platô e as novas combinações.
- Posições posteriores continuam no total observado e aparecem separadas como `new_tail_pcs`; elas não aumentam a prioridade de outra escolha. As posições sem linha dentro do intervalo do mapa continuam uma lacuna própria.
- O trabalho continua em uma cópia local do bloco. Esta etapa não fez nova leitura do contrato nem chamou IA.

## Conferência no VPS

- `python3 -m py_compile tools/wcop_adaptive.py`: passou.
- `cargo check -p security-engine --offline`: passou.
- `cargo build --release -p security-engine --offline`: passou como root, dono dos artefatos anteriores. A primeira tentativa com o usuário toupera parou por permissão no arquivo de build existente, sem erro no código.
- Reprocessamento offline dos arquivos privados do trabalho `7d78331aa23a4592`: fim do mapa PC 8499, 37 posições após o mapa, zero observadas. O Rust escolheu uma próxima opção com nove posições mapeadas ainda não vistas no alcance estimado.
- Conferência de limite: ao marcar todas as posições até o fim do mapa como vistas em uma cópia temporária do estado, `reachable_unseen_pcs` ficou zero e as 37 posições finais permaneceram registradas separadamente.

## Limites e próximo passo

- O mapa do compilador não demonstra que uma posição seja alcançável. A escolha também depende de candidatos e estados disponíveis.
- A medição retrospectiva confirma separação contábil e de prioridade. Ela não mede aumento de cobertura nem confirma uma diferença no contrato.
- Próxima etapa: classificar posições sem linha dentro do mapa e relacionar as novas passagens do runtime à medição de ramos da fonte, preservando denominadores distintos.

## Rodada integrada posterior

- Trabalho `92273047cbf7439c`, bloco `26228846`: 38 escolhas, 544 PCs novos dentro do mapa, zero depois do limite e parada por platô.
- O total observado continuou em 3014/5377 PCs, igual ao trabalho anterior em outro bloco. A etapa melhorou a contabilidade da escolha; não houve ganho de cobertura demonstrado. Ver `WCOP-RODADA-92273047cbf7439c.md`.
