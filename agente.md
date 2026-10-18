# Memória operacional — Toupera Agent

## Objetivo atual
Criar a base de um auditor de segurança econômica para Solidity/EVM, focado em vaults, oracles, accounting, conversões de shares/assets, fees e rounding.

## Arquitetura vigente
Reconstrução econômica do contrato em Rust, por source e futuramente bytecode EVM (fluxos, storage, chamadas, oracles, pricing e shares/assets) → filtragem determinística (AST/Slither) → redução de candidatos e context slicing → raciocínio LLM → hipótese econômica → validação local reproduzível com Foundry → evidências e revisão humana. Inferências precisam de proveniência e nível de confiança; semântica ambígua permanece marcada como incerta. A IA não recebe o repositório inteiro por padrão e não confirma vulnerabilidades sem reprodução.

## Fase atual
M1 — IR econômica com proveniência concluída. M2 (extração source/AST) é o próximo milestone.

## Decisões técnicas
- Core Rust em workspace Cargo, sem dependências externas no crate inicial.
- README e documentação mantêm o escopo restrito a Solidity/EVM, vaults e oracles.
- IDs como newtypes `String`; operações e classificações como enums.
- O core Rust também deverá reconstruir o modelo econômico do contrato; source-first no início, bytecode-only como extensão planejada com evidências e incertezas explícitas.
- `EconomicFunction` registra operação, storage, chamadas, oracles, aritmética e incertezas; cada observação tem localização e confiança. A fixture M1 monta esses dados manualmente; extração automática ainda não existe.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados/criados
- `README.md`, `agente.md`, `.gitignore`, `Cargo.toml`, `Cargo.lock`
- `codex/agent.md`
- `docs/ARCHITECTURE.md`, `docs/ECONOMIC-IR.md`, `docs/ECONOMIC-SCOPE.md`, `docs/SECURITY-SCOPE.md`, `docs/ROADMAP-CHECKLIST.md`
- `assets/toupera-agent.png`
- `crates/security-engine/Cargo.toml`, `crates/security-engine/src/lib.rs`, `crates/security-engine/src/ir.rs`, `crates/security-engine/tests/economic_ir.rs`
- `tests/fixtures/VaultWithOracle.sol`

## Testes executados
`cargo test -p security-engine` (18/10/2026): 2 testes unitários passaram; doc-tests: 0 testes, sem falhas.
Esta atualização mudou apenas documentação; `git diff --check` passou. Os testes Rust não foram reexecutados.
Checklist de construção e auditoria adicionada em `docs/ROADMAP-CHECKLIST.md`; testes não reexecutados nesta etapa documental.
M1: `cargo fmt --all` e `cargo test -p security-engine` executados; 5 testes passaram, 0 falhas; doc-tests: 0 testes.

## Resultados
M1: IR de função econômica compilada sem dependências novas. Cinco testes passaram, incluindo resolução de source spans, limites UTF-8 e registro de evidências do vault fixture.

## Erros conhecidos
Nenhum erro conhecido no M1. O teste monta a IR manualmente; ingestão solc AST, reconstrução de bytecode, detectores e integração Foundry ainda não foram implementados.

## Próximo passo
M2 recomendado: ingerir solc AST de um vault autorizado/fixture e preencher a IR automaticamente, preservando source spans; começar por operações e dependências diretas antes de montar CFG completo.

## Pendências
- Integrações e detectores seguem pendentes para milestones posteriores.
- Reconstrução bytecode-only (CFG, selector dispatch, storage/call inference e proxy mapping) permanece planejada; ainda não implementada.
