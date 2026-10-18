# Memória operacional — Toupera Agent

## Objetivo atual
Construir um auditor de segurança econômica Solidity/EVM para vaults, oracles, accounting, shares/assets, pricing, fees e rounding. O motor Rust deve reconstruir o modelo econômico do contrato antes de formular hipóteses.

## Arquitetura vigente
Alvo autorizado → source/bytecode → reconstrução Rust com proveniência e incertezas → filtros determinísticos/Slither → context slicing → LLM → hipótese → teste local Foundry → evidência → revisão humana → relatório. Source e runtime EVM legado explícito já têm recortes de reconstrução separados. Nenhuma hipótese vira vulnerabilidade sem reprodução.

## Fase atual
M0, M1 e M2 concluídos no recorte source-first de um arquivo sem imports. M3 concluído no recorte conservador de runtime EVM legado explícito, com CFG de destinos constantes, fluxo abstrato de stack/memória por palavras, dependências genéricas de valor e comparação opcional com runtime compilado local. EVMole 0.9.4 fornece uma segunda leitura local de selectors, ABI, storage e CFG em relatório separado. O motor não atribui identidade de oracle/assets/shares apenas pelo bytecode.

## Decisões técnicas
- Core Rust em workspace Cargo; `serde`/`serde_json` apenas para AST e IR JSON.
- `solc --standard-json` é executado localmente sobre arquivo fornecido explicitamente; sem imports nesta etapa.
- IR registra operação, storage, chamadas, oracle, aritmética, condições, modifiers e incertezas com source spans e confiança.
- `delegatecall` produz candidato tentativo quando o alvo é desconhecido. `ProxyRelation` só liga implementação local criada no construtor e guardada em endereço `immutable`, com spans da atribuição, tipo e receptor.
- O grafo source-level liga `entry`/`exit` a nós AST e preserva source spans por aresta. Desvios `if`, `require`/`assert`, ternário simples, `while`, `for`, `do while`, `break`, `continue`, retorno e revert explícito são modelados; construções não suportadas ficam em `unresolved`, sem inventar sucessores.
- Modifiers definidos no mesmo arquivo são ligados pela referência de declaração do AST. A IR guarda invocação, definição, bindings parâmetro→argumento e efeitos sintáticos em `applied_modifiers`. Com exatamente um `_`, o CFG percorre os modifiers na ordem declarada e seus pós-trechos em ordem inversa, inclusive após `return` do corpo da função.
- `ReachabilityPath` registra chamadas internas do mesmo arquivo, inclusive bases até leituras/escritas de storage, candidatos de oracle e conversões, preservando o span de cada chamada. Alcance não comprova fluxo de valor.
- `ValueFlow` liga origem (parâmetro, storage, oracle, chamada externa) a guarda, retorno, argumento de conversão ou escrita de storage; conserva spans e chamadas contribuintes com confiança inferida. Propaga locals, argumentos/retornos internos e atualizações de storage. Não é execução simbólica.
- Remoto `origin` privado: `https://github.com/glaudsonvilela/Toupera-Agent`.

## Arquivos alterados nesta etapa
- Fechamento M2: `crates/security-engine/src/{source,ir,lib}.rs`, fixture `ResolvedProxyFixture`, teste de alvo imutável/mutável, README e documentação.
- Herança no source AST: `crates/security-engine/src/{source,callgraph,dataflow,ir,lib}.rs`, fixture `InheritedVaultFixture` e teste de percurso oracle→conversão→supply.
- `README.md`, `agente.md`, `codex/agent.md` e `docs/{ECONOMIC-IR,ROADMAP-CHECKLIST,SOURCE-RECONSTRUCTION}.md`.

## Testes executados
- Fechamento M2: `cargo fmt --all`, `cargo test -p security-engine --offline` e `git diff --check`: 22 testes passaram, 0 falhas. CLI com solc local 0.8.37 executada nas fixtures de herança e proxy fixo.
- `cargo test --workspace --offline`: 22 testes passaram. Clippy com `-A clippy::collapsible_if -D warnings` passou; a execução estrita sem essa exceção encontrou 13 avisos de estilo `collapsible_if` já fora do critério funcional M2.
- Após herança: `cargo fmt --all` e `cargo test -p security-engine --offline`: 20 testes passaram.
- CLI executada também em `VaultWithOracle.sol` e `VaultCallFlowFixture.sol`; a primeira fixture liga oracle apenas à guarda, a segunda ao argumento de `convertToShares` e à escrita de `totalSupply`.

## Resultados
M2 liga base e derivado pelo AST, conserva identidade das chamadas herdadas e distingue alvo de proxy fixo local de endereço externo incerto. Fluxo de valor separa `oracle.latestPrice → require` (sem caminho às shares) de `oracle.latestPrice → readPrice → deposit → convertToShares → totalSupply` na fixture de preço. Os modifiers locais com um `_` também propagam parâmetros à guarda. São inferências estáticas conservadoras, não achados de vulnerabilidade.

## Erros conhecidos / limites
Sem suporte a imports, herança em outros arquivos, CFG completo ou resolução de endereço de proxy fornecido na implantação. O suporte a bytecode é somente runtime legado conservador, não EOF/creation code nem decompilação completa. Loops de source têm aproximação de uma iteração com incerteza; chamadas desconhecidas não têm retorno presumido. Fluxo de valor não prova factibilidade de caminho. Modifiers ausentes da fonte ou com zero/múltiplos `_` ficam sem composição. `try`, assembly e ternários múltiplos/aninhados não têm fluxo completo.

## Próximo passo
Próximo marco M4: detectores econômicos determinísticos sobre as IRs existentes; medir sinais em fixtures antes de associar qualquer hipótese a vulnerabilidade.

## Etapa M3 inicial (2026-10-18)
- `crates/security-engine/src/bytecode.rs` analisa runtime EVM legado em hex. Remove metadados CBOR Solidity somente com estrutura completa e chave reconhecida; decodifica `PUSH` sem confundir dados imediatos com opcodes, cria blocos, resolve apenas saltos literais a `JUMPDEST`, reconhece candidatos de dispatch direto `PUSH4/EQ/PUSH/JUMPI` (`Inferred` só com `DUP1` anterior e alcance conhecido) e observa storage/transient storage/chamadas/criação/autodestruição por PC.
- `entry_summaries` reúne efeitos alcançados por cada selector recuperado. Blocos não alcançados pelo CFG conhecido mantêm confiança `Tentative`; saltos dinâmicos, `PUSH` truncado, opcodes desconhecidos e disponibilidade dependente de hard fork ficam em `unresolved`. Um slot constante só é ligado a `SLOAD`/`SSTORE`/`TLOAD`/`TSTORE` quando vem de `PUSH` imediatamente anterior, com confiança `Inferred`.
- CLI: `--bytecode <runtime.hex>` e `--compare-bytecode <runtime.hex> <solc.json> <source> <contrato>`. A comparação opcional usa runtime normalizado e `methodIdentifiers` de saída local do solc; selector isolado não comprova identidade do source. EOF, creation code, saltos calculados, stack/memória e fluxo econômico não são recuperados neste recorte.
- Documentação: `docs/BYTECODE-RECONSTRUCTION.md`, roadmap, IR e README atualizados. `cargo fmt --all`, `cargo check -p security-engine --offline` e `git diff --check` executados; nenhum teste automatizado foi executado nesta etapa.
- A revisão `3d2a9a6` foi enviada ao GitHub privado, sincronizada no VPS e compilada em release com `cargo build --release --locked -p security-engine` sob o usuário `toupera`; `.deployed-commit` registrou a revisão. O CLI de bytecode não foi executado em fixture nesta etapa.

## Decisão de ferramentas após M2 (2026-10-18)
- Avaliação documentada em `docs/TOOLING-STRATEGY.md`. Na etapa original de avaliação, nenhum scanner, fuzzer ou provador adicional havia sido instalado ou integrado; a instalação posterior no VPS está registrada abaixo.
- Prioridade: Rust + Slither para triagem; Foundry para invariantes e reprodução; Halmos em propriedades curtas. Echidna **ou** Medusa apenas se houver lacuna de cobertura ou achados únicos. Mythril pode apoiar M3 seletivamente; Securify2 não é prioridade e Manticore está arquivado.
- Certora e Kontrol são opções para provas de alto impacto. O CLI padrão de Certora envia jobs aos servidores do fornecedor; alvo privado exige autorização específica antes de qualquer envio.
- Redução de uso de IA depende de gates e deduplicação; medir tokens, candidatos e bugs reproduzidos. Não afirmar ganho numérico sem benchmark. IA recebe apenas slices de candidatos ainda ambíguos.
- Esta etapa alterou documentação e memórias; não alterou código nem executou ferramentas de auditoria em alvos.

## Infraestrutura informada pelo usuário
- VPS Hostinger KVM 4 `srv2074743.hstgr.cloud`, Ubuntu 26.04.1 LTS, 4 vCPU, 16 GiB RAM e 200 GB de disco; acesso SSH root verificado com a chave pública ED25519 autorizada pelo usuário.
- DeepSeek será usado por API. Inferência não será hospedada no VPS; enviar apenas slices mínimos e nunca credenciais ou o repositório inteiro por padrão.
- Pacotes Ubuntu atualizados. Usuário de execução `toupera` sem sudo, UFW restrito a OpenSSH, Fail2ban SSH e swap de 16 GB ativos. Atualizações automáticas de segurança já estavam ativas.
- Reinicialização pós-atualização concluída; SSH, Fail2ban, UFW, swap, `solc`, Rust, Slither, Foundry e binário Rust verificados de novo. Nenhuma reinicialização pendente.
- Rust/Cargo 1.93.1, solc 0.8.37 com SHA-256 conferido, Slither 0.11.6 e Foundry 1.8.5 instalados. Fonte privada enviada por `git archive` a `/srv/toupera-agent`, sem token GitHub; CLI Rust compilado em release com `--locked`.
- Smoke operacional: CLI produziu 4 registros de IR em `VaultCallFlowFixture.sol`; Slither analisou a fixture com 102 detectores e 3 alertas (`success=true`); `forge --version` funcionou. Não foram executados testes automatizados nesta etapa de infraestrutura.
- Nenhum serviço web, API, integração DeepSeek ou secret foi implantado. O SSH por senha/root existente permaneceu habilitado; hardening adicional de autenticação exige coordenação para evitar perda de acesso. Ver `docs/VPS-SETUP.md`.
- Limitar paralelismo de fuzzing e escalonar provas simbólicas conforme métricas reais de CPU, memória e disco.

## Instalação adicional de ferramentas (2026-10-18)
- A pedido do usuário, os 10 executores da estratégia estão disponíveis: Slither 0.11.6, Mythril 0.24.8, Securify2 (commit `1913dfe`), Manticore 0.3.7, Foundry 1.8.5, Echidna 2.3.3, Medusa 1.5.1, Certora CLI 8.20.2, Halmos 0.3.3 e Kontrol 1.0.255. Inventário e métodos em `docs/VPS-SETUP.md`.
- Python 3.10 e 3.12 isolados por uv; Java 21 para Certora; Podman rootless para Securify2, Manticore e Kontrol. Securify2 compilou com `graphviz==0.19.2` fixado apenas na cópia da ferramenta e com solc 0.5.12. Manticore e Securify2 são legados; não presumir compatibilidade com contratos modernos.
- Comandos de versão/ajuda foram executados para os CLIs; Securify2 foi construído e abriu `--help`. Nenhum teste automatizado do Toupera Agent foi executado. Nenhuma análise em alvo real, prova Certora ou envio de source privado a terceiros foi realizado.
- O CLI Certora não tem `CERTORAKEY`; instalação não habilita provas remotas. As ferramentas ainda não estão ligadas à IR Rust ou ao pipeline do projeto.

## Fechamento M3 — recorte legado (2026-10-18)
- `crates/security-engine/src/bytecode_flow.rs` propaga stack e palavras de memória de 32 bytes por blocos e arestas conhecidas, com limites de iteração, origens e fluxos. Resolve destinos constantes adicionais de `JUMP`/`JUMPI` e registra relações PC→PC entre calldata, callvalue, storage, retorno/sucesso de chamada e balance até storage, valor/alvo/entrada de chamada, retorno e condição; PCs aritméticos acompanham a dependência.
- `BytecodeAnalysis.value_flows` e `BytecodeEntrySummary.value_flows` são inferências estáticas. No resumo por selector, confiança é `Tentative` por possível compartilhamento de blocos. Memória parcial, ABI desconhecida, salto dinâmico, stack incompatível, opcodes não modelados e orçamento esgotado são incertezas ou limites; não há detecção de bug nesta etapa.
- `compare_with_solc_contract` atribui `EconomicOperation` por nome de método somente quando o runtime normalizado do solc coincide byte a byte. Um selector isolado nunca recebe rótulo econômico de source divergente. Documentação e checklist M3 atualizados em `docs/BYTECODE-RECONSTRUCTION.md` e `docs/ROADMAP-CHECKLIST.md`.
- `cargo fmt --all` e `cargo check -p security-engine --offline` executados durante a implementação. Nenhum teste foi adicionado ou executado nesta etapa.

## Pendências
Detectores econômicos M4, deduplicação, LLM e juiz Foundry permanecem planejados. Nunca armazenar secrets ou executar exploração em produção.

## Complemento M3 — EVMole como evidência independente (2026-10-18)
- Dependência Rust `evmole = =0.9.4`, com recursos padrão desligados, integra a análise de runtime legado já normalizado em `crates/security-engine/src/bytecode_evmole.rs`. `BytecodeAnalysis.evmole` registra selectors ABI/fallback, tipos prováveis de argumentos, mutabilidade, layout de storage persistente/transitório, CFG, versão, confiança e divergências. O processamento é local; não há consulta a rede durante a análise.
- Propostas do EVMole não entram automaticamente em `control_edges`, `effects` nem `value_flows` nativos. Selector só é aceito se sua entrada aponta a um `JUMPDEST` decodificado; tipos, storage e arestas são `Tentative`. Slots EVMole só ganham `corroborating_access_pcs` quando um efeito nativo com slot literal e selector alcançável coincide. Ausência de PC de corroboração não prova ausência do acesso.
- `compare_with_solc_contract` pode considerar selector ABI exclusivo do EVMole; `recovered_by` mostra `native`, `evmole` ou ambos. EVMole exclusivo e divergência de destino mantêm confiança `Tentative`; rótulo econômico pelo nome do método exige igualdade do runtime compilado normalizado. A comparação agora normaliza o runtime do solc sem reexecutar toda a análise.
- O companion registra motivo de omissão acima de 24.576 bytes e preserva a análise nativa caso o EVMole falhe. Heimdall e Gigahorse não foram integrados. EVMole não confirma vulnerabilidades nem prova ABI/layout.
- `cargo check -p security-engine --offline` e `cargo fmt --all` foram usados para compilação e formatação. Nenhum teste automatizado foi adicionado ou executado nesta etapa.
