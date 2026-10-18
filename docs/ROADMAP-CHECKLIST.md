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

### M2 — Reconstrução source-first (concluído no recorte de um arquivo)

- [x] Ingerir um arquivo Solidity explícito e obter AST do solc local.
- [x] Construir fluxos de controle, chamadas e dependências de estado.
- [x] Mapear operações econômicas, caminhos de oracle e conversões shares/assets.
- [x] Identificar proxies/implementações quando houver evidência.
- [x] Comparar a IR com fixtures e resultados locais conhecidos.

O recorte M2 cobre um arquivo Solidity sem imports. O grafo estrutural source-level liga `if`, retornos, `require`/`assert`, ternários simples, laços e modifiers com um `_`, inclusive herdados no mesmo arquivo. Chamadas internas preservam o contrato declarante; caminhos e fluxo de valor conservador chegam a storage, oracle e conversões com proveniência. `delegatecall` com implementação `immutable` criada localmente no construtor é ligado ao contrato alvo; endereços externos continuam candidatos incertos. Laços e chamadas desconhecidas mantêm incertezas. `try`, assembly, múltiplos ternários, imports e bytecode permanecem fora deste recorte; M3 cobre bytecode.

### M3 — Engenharia reversa de bytecode EVM (concluído no recorte legado conservador)
- [x] Normalizar runtime hex legado e remover metadados CBOR Solidity apenas quando reconhecidos.
- [x] Recuperar dispatch direto de selectors, blocos básicos e CFG de saltos literais ou constantes propagadas.
- [x] Observar chamadas e storage por PC; inferir dependências genéricas entre entradas, leituras, retornos de chamadas, aritmética e sinks econômicos.
- [x] Registrar observações e inferências recuperadas com PC, confiança e incertezas.
- [x] Manter comportamento ambíguo como desconhecido; comparar runtime normalizado e métodos com saída local do solc quando disponível.

O fechamento M3 cobre somente runtime EVM legado fornecido explicitamente: interpretação abstrata limitada de stack e memória por palavras, com limite de propagação. Os caminhos econômicos são dependências genéricas até escrita de storage, valor/alvo/entrada de chamada, condição ou retorno, sem atribuir identidade de oracle, asset ou share ao bytecode isolado. Dispatch fora do padrão direto, saltos ainda dinâmicos, memória parcial, ABI desconhecida, EOF e creation code permanecem em `unresolved` ou fora do recorte. A comparação local com source só atribui rótulos econômicos quando o runtime normalizado coincide byte a byte. Não é decompilador completo nem detector de vulnerabilidade. Ver [reconstrução de bytecode](BYTECODE-RECONSTRUCTION.md).

Complemento após M3: EVMole 0.9.4 fornece uma segunda leitura local de selectors, argumentos, mutabilidade, storage e CFG. Suas propostas permanecem separadas e tentativas; divergências são registradas, não incorporadas automaticamente ao grafo nativo. A comparação com source pode aproveitar selector ABI exclusivo do EVMole, ainda com confiança `Tentative`.

Complemento adicional: Heimdall ABI e Gigahorse TAC podem ser importados com `--bytecode-companions` ou gerados pelo wrapper local. Selectors são correlacionados com PCs nativos/EVMole; blocos e sucessores TAC são correlacionados com PCs decodificados. Nenhuma dessas propostas entra automaticamente em fluxo econômico ou achado de vulnerabilidade.

### M4 — Análise econômica determinística (implementação source-backed; calibração pendente)
- [x] Codificar triagem estrutural de conservação assets/shares e assimetria de preço, sem alegar prova numérica.
- [x] Emitir candidatos para escalas decimais, arredondamento, preview/execução e accounting com evidências observadas.
- [x] Emitir candidatos para sensibilidade de primeira emissão/doação, fee e caminhos de oracle sob premissas explícitas.
- [x] Importar JSON local do Slither como evidência complementar e deduplicar por contrato, função e categoria na IR Rust.
- [x] Emitir apenas candidatos `UNVERIFIED` com spans, regras, premissas e confiança tentativa.
- [ ] Medir redução de candidatos, falsos positivos e cobertura em fixtures: adiado a pedido do usuário até a implementação completa.

O M4 usa a IR source-first de um arquivo sem imports. Regras sintáticas não provam conservação de quantidades, round trips ERC-4626, possibilidade de doação ou qualidade de oracle. O bytecode M3 permanece evidência estrutural separada. A saída e os limites estão em [Análise Econômica](ECONOMIC-ANALYSIS.md).

### M5 — Deduplicação e contexto mínimo
- [x] Calcular hashes de source, runtime opcional normalizado, AST e função, além de compiler e motor.
- [x] Evitar reanálise de entradas idênticas com cache local por conteúdo.
- [x] Montar slices limitados com função, chamadas, storage, aritmética, oracle e invariantes relevantes.
- [x] Manter saída local sem envio ao LLM ou do repositório inteiro por padrão.

O M5 opera no recorte source-backed do M2/M4, um arquivo sem imports. Runtime opcional é só fingerprint; não é ligado ao source sem comparação exata. O cache privado guarda o pacote reduzido e invalida ao mudar entrada, compilador ou motor. Os limites e o formato estão em [Pacote de Contexto](CONTEXT-PACKAGE.md). Economia de tokens e taxa de acertos permanecem sem benchmark até a validação solicitada após a implementação completa.

### M6 — Hipóteses do LLM
- [ ] Definir interface desacoplada do fornecedor.
- [ ] Enviar somente contexto reduzido e estruturado.
- [ ] Exigir hipótese, premissas, caminhos relevantes e teste sugerido.
- [ ] Manter o achado como `UNVERIFIED`; LLM não confirma vulnerabilidade.
- [ ] Medir chamadas/tokens e acionar IA apenas para candidatos deduplicados que ainda exigem interpretação.

### M7 — Juiz Foundry
- [ ] Gerar teste local reproduzível para cada hipótese priorizada.
- [ ] Executar apenas em unit tests, Anvil ou fork local autorizado.
- [ ] Aplicar fuzzing de Foundry a limites, fees e arredondamentos.
- [ ] Rejeitar hipóteses que não reproduzem o comportamento alegado.
- [ ] Reexecutar resultados positivos e guardar traces relevantes.
- [ ] Medir cobertura, chamadas válidas e reverts; avaliar Echidna ou Medusa apenas quando acrescentarem alcance ou achados únicos.
- [ ] Piloto Halmos para propriedades curtas; avaliar Kontrol ou Certora somente para provas críticas com premissas explícitas.

Critérios e prioridade por ferramenta: [Estratégia de ferramentas](TOOLING-STRATEGY.md).

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
