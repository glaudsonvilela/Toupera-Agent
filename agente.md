# Memória operacional — Toupera Agent

## Objetivo atual
Criar a base de um auditor de segurança econômica para Solidity/EVM, focado em vaults, oracles, accounting, conversões de shares/assets, fees e rounding.

## Arquitetura vigente
Filtragem determinística (Rust/AST/Slither) → redução de candidatos e context slicing → raciocínio LLM → hipótese econômica → validação local reproduzível com Foundry → evidências e revisão humana. A IA não recebe o repositório inteiro por padrão e não confirma vulnerabilidades sem reprodução.

## Fase atual
M0 — Repository/Foundation concluído.

## Decisões técnicas
- Core Rust em workspace Cargo, sem dependências externas no crate inicial.
- README e documentação mantêm o escopo restrito a Solidity/EVM, vaults e oracles.
- IDs como newtypes `String`; operações e classificações como enums.
- Repositório inicial local; nenhum remoto foi configurado.

## Arquivos alterados/criados
- `README.md`, `agente.md`, `.gitignore`, `Cargo.toml`, `Cargo.lock`
- `codex/agent.md`
- `docs/ARCHITECTURE.md`, `docs/ECONOMIC-SCOPE.md`, `docs/SECURITY-SCOPE.md`
- `assets/toupera-agent.png`
- `crates/security-engine/Cargo.toml`, `crates/security-engine/src/lib.rs`

## Testes executados
`cargo test -p security-engine` (18/10/2026): 2 testes unitários passaram; doc-tests: 0 testes, sem falhas.

## Resultados
`cargo test -p security-engine` (18/10/2026): 2 testes unitários passaram; doc-tests: 0 testes, sem falhas.

## Erros conhecidos
Nenhum conhecido no M0. Detectores e integração com solc, Slither, Alloy e Foundry ainda não foram implementados.

## Próximo passo
M1 recomendado: definir entrada e saída da IR mínima para funções econômicas de vault, com fixtures Solidity pequenas e sem ainda adicionar análise complexa.

## Pendências
- Nenhum repositório remoto fornecido/localizado; configurar apenas se um remoto privado existente for identificado e autorizado.
- Integrações e detectores seguem pendentes para milestones posteriores.
