# Checklist — da fundação ao relatório

Esta lista acompanha a construção do Toupera Agent e o ciclo completo de análise de cada alvo. Uma hipótese só avança com evidência suficiente.

## Construção do motor

### M0 — Fundação
- [x] Repositório privado, estrutura, documentação e imagem oficial.
- [x] Crate Rust com tipos fundamentais e testes mínimos.

### M1 — IR econômica com proveniência
- [x] Definir IDs de contrato, função e localizações de evidência.
- [x] Representar operações deposit/mint/withdraw/redeem, previews e conversões.
- [x] Representar storage lido/escrito, chamadas, oracles e expressões aritméticas.
- [x] Associar cada elemento ao source span e preparar PC de bytecode para etapa futura.
- [x] Marcar confiança, semântica incerta e dados ausentes explicitamente.
- [x] Validar a IR com fixtures pequenas de vaults Solidity.

### M2 — Reconstrução source-first (em andamento)
- [x] Ingerir um arquivo Solidity explícito e obter AST do solc local.
- [ ] Construir fluxos de controle, chamadas e dependências de estado.
- [ ] Mapear operações econômicas, caminhos de oracle e conversões shares/assets.
- [ ] Identificar proxies/implementações quando houver evidência.
- [x] Comparar a IR com fixtures e resultados locais conhecidos.

O recorte atual captura condições de desvio, chamadas internas, acessos diretos a storage e candidatos de oracle/proxy. O grafo estrutural source-level já liga `if`, retornos, `require`/`assert`, ternários simples, laços e modifiers locais com um `_`, preservando a ordem de execução. Os argumentos dos modifiers têm vínculo sintático com os parâmetros. Faltam `try`/assembly, dataflow entre funções, herança e resolução mais forte de proxy para fechar as três caixas restantes.

### M3 — Engenharia reversa de bytecode EVM
- [ ] Normalizar bytecode e remover metadados quando possível.
- [ ] Recuperar selector dispatch, blocos básicos e fluxo de controle.
- [ ] Inferir chamadas externas, acessos a storage e caminhos econômicos.
- [ ] Registrar cada inferência com PC, confiança e incertezas.
- [ ] Manter comportamento ambíguo como desconhecido; comparar com source quando disponível.

### M4 — Análise econômica determinística
- [ ] Codificar conservação de assets/shares e simetria de preço.
- [ ] Detectar mismatch de decimals, rounding, preview/execução e accounting.
- [ ] Detectar share inflation, donation manipulation, fee bypass e problemas de oracle.
- [ ] Emitir candidatos com evidências, nunca veredictos automáticos.
- [ ] Medir redução de candidatos e falsos positivos em fixtures.

### M5 — Deduplicação e contexto mínimo
- [ ] Calcular hashes de source, bytecode, AST e função.
- [ ] Evitar reanálise de código conhecido.
- [ ] Montar slices com função, chamadas, storage, aritmética, oracle e invariantes relevantes.
- [ ] Não enviar o repositório inteiro ao LLM por padrão.

### M6 — Hipóteses do LLM
- [ ] Definir interface desacoplada do fornecedor.
- [ ] Enviar somente contexto reduzido e estruturado.
- [ ] Exigir hipótese, premissas, caminhos relevantes e teste sugerido.
- [ ] Manter o achado como `UNVERIFIED`; LLM não confirma vulnerabilidade.

### M7 — Juiz Foundry
- [ ] Gerar teste local reproduzível para cada hipótese priorizada.
- [ ] Executar apenas em unit tests, Anvil ou fork local autorizado.
- [ ] Aplicar fuzzing de Foundry a limites, fees e arredondamentos.
- [ ] Rejeitar hipóteses que não reproduzem o comportamento alegado.
- [ ] Reexecutar resultados positivos e guardar traces relevantes.

### M8 — Evidência e entrega
- [ ] Registrar versão/hash do alvo, configuração e passos para reproduzir.
- [ ] Separar fatos, inferências, premissas e limitações.
- [ ] Fazer revisão humana de escopo, impacto e duplicidade.
- [ ] Preparar relatório claro, com evidência mínima suficiente.
- [ ] Marcar `REPORT_READY` somente após reprodução e revisão.

## Ciclo por alvo auditado

### 1. Autorização e escopo
- [ ] Confirmar propriedade, ambiente de laboratório ou autorização explícita do programa.
- [ ] Registrar limites permitidos e manter análise em Solidity/EVM, vaults, oracles e economia.
- [ ] Não enviar transações ofensivas nem explorar produção.

### 2. Coleta e identificação
- [ ] Registrar endereço/rede ou origem local, versão e hashes disponíveis.
- [ ] Identificar source, bytecode ou ambos; resolver proxy com evidência.
- [ ] Deduplicar artefatos já conhecidos.

### 3. Reconstrução e análise
- [ ] Construir AST ou recuperação de bytecode/CFG conforme a entrada disponível.
- [ ] Mapear funções econômicas, chamadas, modificadores e dependências de storage.
- [ ] Seguir oracle, preço, decimals, fees e rounding até mint/burn e accounting.
- [ ] Verificar previews, operações opostas, primeiro depósito e doações diretas.
- [ ] Marcar incertezas; reduzir candidatos deterministicamente antes do LLM.

### 4. Hipótese e reprodução
- [ ] Escrever hipótese específica com premissas e contexto mínimo.
- [ ] Criar e executar teste Foundry local autorizado.
- [ ] Guardar resultado reproduzível; descartar ou manter como não verificada se falhar.
- [ ] Confirmar novamente qualquer reprodução positiva.

### 5. Revisão e relatório
- [ ] Validar impacto, condições necessárias e limites do achado.
- [ ] Revisar evidências e regras do programa autorizado com uma pessoa.
- [ ] Redigir relatório com passos de reprodução e sem afirmações além das evidências.
- [ ] Marcar `CONFIRMED` e `REPORT_READY` somente após os gates acima.

## Estados do achado

`UNVERIFIED → REJECTED | REPRODUCED → CONFIRMED → REPORT_READY`
