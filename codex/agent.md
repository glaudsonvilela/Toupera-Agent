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

## Estado após M2 — grafo source-level parcial

- Leia `docs/ROADMAP-CHECKLIST.md`, `docs/ECONOMIC-IR.md` e `docs/SOURCE-RECONSTRUCTION.md`.
- `source.rs` extrai IR de solc AST para um arquivo sem imports e resolve corpos de modifiers locais em `applied_modifiers`, com bindings sintáticos de argumento/parâmetro. `cfg.rs` intercala modifiers com um `_` ao grafo da função e preserva pós-trechos após `return`. Testes cobrem cinco fixtures e a CLI foi executada com solc local 0.8.37.
- Próxima etapa: resolver herança, dataflow entre funções e proxy. `try`, assembly, ternários múltiplos e modifiers com zero/múltiplos `_` ainda não têm fluxo completo. Não afirme que CFG completo ou engenharia reversa de bytecode já funcionam.
- Atualize este arquivo e `/agente.md` ao final de cada etapa, como solicitado pelo usuário.
