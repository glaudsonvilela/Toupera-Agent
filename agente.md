# Memória operacional — Toupera Agent

## Objetivo atual
Criar a base de um auditor de segurança econômica para Solidity/EVM, focado em vaults, oracles, accounting, conversões de shares/assets, fees e rounding.

## Arquitetura vigente
Reconstrução econômica do contrato em Rust, por source e futuramente bytecode EVM (fluxos, storage, chamadas, oracles, pricing e shares/assets) → filtragem determinística (AST/Slither) → redução de candidatos e context slicing → raciocínio LLM → hipótese econômica → validação local reproduzível com Foundry → evidências e revisão humana. Inferências precisam de proveniência e nível de confiança; semântica ambígua permanece marcada como incerta. A IA não recebe o repositório inteiro por padrão e não confirma vulnerabilidades sem reprodução.

## Fase atual
M0 — Repository/Foundation concluído.

## Decisões técnicas
- Core Rust em workspace Cargo, sem dependências externas no crate inicial.
- README e documentação mantêm o escopo restrito a Solidity/EVM, vaults e oracles.
- IDs como newtypes `String`; operações e classificações como enums.
- O core Rust também deverá reconstruir o modelo econômico do contrato; source-first no início, bytecode-only como extensão planejada com evidências e incertezas explícitas.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados/criados
- `README.md`, `agente.md`, `.gitignore`, `Cargo.toml`, `Cargo.lock`
- `codex/agent.md`
- `docs/ARCHITECTURE.md`, `docs/ECONOMIC-SCOPE.md`, `docs/SECURITY-SCOPE.md`
- `assets/toupera-agent.png`
- `crates/security-engine/Cargo.toml`, `crates/security-engine/src/lib.rs`

## Testes executados
`cargo test -p security-engine` (18/10/2026): 2 testes unitários passaram; doc-tests: 0 testes, sem falhas.
Esta atualização mudou apenas documentação; `git diff --check` passou. Os testes Rust não foram reexecutados.

## Resultados
`cargo test -p security-engine` (18/10/2026): 2 testes unitários passaram; doc-tests: 0 testes, sem falhas.

## Erros conhecidos
Nenhum conhecido no M0. Detectores e integração com solc, Slither, Alloy e Foundry ainda não foram implementados.

## Próximo passo
M1 recomendado: definir uma IR econômica com proveniência para funções de vault, começar pelo caminho source/AST e preparar o contrato para ingestão futura de bytecode; usar fixtures Solidity pequenas, sem ainda implementar decompilação complexa.

## Pendências
- Integrações e detectores seguem pendentes para milestones posteriores.
- Reconstrução bytecode-only (CFG, selector dispatch, storage/call inference e proxy mapping) permanece planejada; ainda não implementada.
