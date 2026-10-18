# Instruções para futuras sessões do Codex

1. Leia primeiro `/agente.md`.
2. Trabalhe em etapas pequenas.
3. Não faça grandes refactors sem necessidade.
4. Não generalize o escopo: priorize Solidity/EVM, Vaults e Oracles.
5. Trate o motor Rust como analisador econômico e motor de reconstrução do contrato; planeje suporte a source e, depois, bytecode EVM, sem afirmar inferências incertas como fatos.
6. Execute os testes relevantes após cada mudança.
7. Atualize `/agente.md` depois de cada etapa importante.
8. Nunca armazene API keys, tokens, passwords, secrets, private keys ou seed phrases.
9. Não modifique outros projetos no SSD Sherlock.
10. Prefira análise determinística antes de LLM e envie somente contexto mínimo.
11. Minimize tokens e contexto.
12. Não invente resultados: registre apenas verificações realmente executadas.
13. Uma vulnerabilidade só é válida após reprodução.
14. Não execute exploração em produção nem envie transações ofensivas para contratos reais.

## Estado após M1

- Leia também `docs/ROADMAP-CHECKLIST.md` e `docs/ECONOMIC-IR.md` antes do próximo milestone.
- A IR Rust de `crates/security-engine/src/ir.rs` está pronta para receber observações com proveniência e confiança; a fixture de teste ainda é montada manualmente.
- Próxima etapa: extrair operações e dependências de solc AST para a IR. Não afirme que a extração ou a engenharia reversa de bytecode já funcionam.
