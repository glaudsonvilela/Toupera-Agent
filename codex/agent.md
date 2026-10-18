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

## Estado após M2 — recorte source-first concluído

- Leia `docs/ROADMAP-CHECKLIST.md`, `docs/ECONOMIC-IR.md` e `docs/SOURCE-RECONSTRUCTION.md`.
- `source.rs` extrai IR de solc AST para um arquivo sem imports e resolve modifiers locais e herdados do mesmo arquivo. `cfg.rs` intercala modifiers com um `_`. `callgraph.rs` produz `ReachabilityPath`. `dataflow.rs` produz `ValueFlow` conservador entre funções, distinguindo oracle usado só em guarda de oracle passado à conversão. Testes cobrem oito fixtures; CLI executada com solc local 0.8.37. `cargo test -p security-engine --offline`: 22 testes passaram; `git diff --check` limpo.
- M2 resolveu herança no mesmo arquivo e `ProxyRelation` para alvo `immutable` criado localmente. Endereço de implantação ou campo mutável continua incerto. Próxima etapa: M3, bytecode EVM. Loops têm aproximação de uma iteração; chamadas desconhecidas não propagam retorno presumido. `try`, assembly, ternários múltiplos e modifiers com zero/múltiplos `_` ainda não têm fluxo completo. Não afirme que CFG completo ou engenharia reversa de bytecode já funcionam.
- Clippy estrito aponta 13 avisos `collapsible_if`; Clippy com essa regra liberada e `-D warnings` passou. Limpeza de estilo fica separada do fechamento funcional do M2.
- Atualize este arquivo e `/agente.md` ao final de cada etapa, como solicitado pelo usuário.

## M3 em andamento — primeira etapa de bytecode legado

- Leia `docs/BYTECODE-RECONSTRUCTION.md`. O módulo `bytecode.rs` produz `BytecodeAnalysis` separado da IR source, com normalização hex/CBOR, disassembly, blocos, saltos literais, dispatch direto, efeitos por PC, resumo por selector e incertezas explícitas. O CLI oferece `--bytecode` e `--compare-bytecode` com saída local do solc.
- Limites: EOF, creation code, saltos dinâmicos, stack/memória, alvos de chamada e caminhos de valor econômico não estão resolvidos. Selector match sem igualdade de runtime é `Tentative`; não confunda observação de opcode com vulnerabilidade.
- `cargo fmt --all`, `cargo check -p security-engine --offline` e `git diff --check` foram executados. Nenhum teste automatizado foi executado nesta etapa. Próximo passo: propagação conservadora de stack e dependências econômicas para continuar M3.
- A revisão `3d2a9a6` também foi enviada ao VPS e compilada em release com Rust 1.93.1. A documentação operacional em `docs/VPS-SETUP.md` foi atualizada para os novos modos do CLI.

## Estratégia de ferramentas após M2

- Leia `docs/TOOLING-STRATEGY.md` antes de integrar scanner, fuzzer ou provador. A análise comparativa foi documentada, sem instalação ou execução dessas ferramentas em alvos.
- Comece por Slither + IR Rust e invariantes Foundry. Use Echidna ou Medusa somente com ganho medido; Halmos é o primeiro piloto simbólico. Mythril é seletivo após M3. Manticore foi arquivado e Securify2 não compõe o fluxo principal.
- Não envie source privado a Certora ou outro serviço externo sem autorização específica; o CLI padrão de Certora submete jobs ao servidor. Registre resultados, versões, premissas, cobertura, contraexemplos e custo de IA, sem inventar percentuais de economia.

## Infraestrutura informada

- O VPS Hostinger KVM 4 foi configurado em 2026-10-18. Leia `docs/VPS-SETUP.md` antes de novas alterações: Ubuntu 26.04.1, 4 vCPU, 16 GiB RAM, disco de 200 GB, SSH funcional, UFW somente OpenSSH, Fail2ban, swap de 16 GB e usuário `toupera` sem sudo.
- Reinicialização após atualizações concluída e acesso SSH, proteção, swap e ferramentas reconfirmados; sem marcador de reinicialização pendente.
- Rust/Cargo 1.93.1, solc 0.8.37, Slither 0.11.6 e Foundry 1.8.5 estão instalados. O snapshot privado está em `/srv/toupera-agent`; o CLI release foi compilado e executado na fixture `VaultCallFlowFixture.sol`. Slither também analisou essa fixture. Ver evidência e limites no documento de VPS.
- DeepSeek será via API, mas não há integração, chave, serviço web ou endpoint implantado. Não alegue que o agente autônomo completo está rodando. Não grave segredos no repositório ou servidor sem mecanismo apropriado.
- O login root e por senha existente não foi desabilitado, para evitar perda de acesso. Alterações futuras de SSH exigem manter uma via administrativa verificada.
- Manter fuzzing com paralelismo limitado até medir consumo. Não armazenar chaves SSH nem chaves da API no repositório.
- Os 10 executores pedidos foram instalados: Slither, Mythril, Securify2, Manticore, Foundry, Echidna, Medusa, Certora CLI, Halmos e Kontrol. Alguns usam Python isolado ou imagens Podman rootless; confirme versões, caminhos, limites e invocação em `docs/VPS-SETUP.md`. Instalação não é integração automática. Certora não possui chave e não deve receber source privado sem autorização específica. Securify2/Manticore são legados.
