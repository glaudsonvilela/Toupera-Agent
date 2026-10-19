# Caminho da leitura automática

```mermaid
flowchart LR
    A[Contrato, rede e bloco fixo] --> B[Identidade do source e runtime]
    B --> C[Motor 1 Rust: mapa de funções, variáveis, storage e PCs]
    T[solc, Slither e leitores de bytecode] --> C
    C --> D{Relação incerta ou diferença?}
    D -- Sim --> E[Motor 2: quarentena e leitura aprofundada]
    E --> C
    D -- Plano local --> F[Cenários no Anvil do VPS]
    F --> G[Estado, recibos, SLOAD/SSTORE e decisões JUMPI]
    G --> C
    G --> K[Triagem local: dependências, variações e parada por platô]
    K --> C
    K -- novos valores e ordens dentro do orçamento --> F
    K --> H[Contexto mínimo para IA, se elegível]
    H --> I[Reprodução Foundry e revisão humana]
    I --> J[Pacote de evidências]
```

O primeiro motor organiza fatos do source, bytecode e histórico. O segundo reexamina apenas uma relação marcada com incerteza ou diferença concreta e devolve o resultado ao mapa. Os cenários usam fork local com bloco e runtime fixos; nenhum envio de transação à rede faz parte desse caminho. A IA recebe contexto limitado somente após a triagem local vinculada por hash; resultados sem diferença repetida ficam no servidor. Uma hipótese só avança com reprodução e revisão.

Na medição privada do YieldVault, 41 dos 110 PCs `JUMPI` estáticos têm origem única em uma função/modifier do source verificado; 69 permanecem sem atribuição. A rodada local teve 36 casos e nenhum resultado novo frente à anterior. Esses números medem observações e limites do mapa, não certificam ausência de problemas nem cobertura de todos os caminhos alcançáveis. Consulte [cadeia de variáveis](VARIABLE-LENS.md) e [checklist](ROADMAP-CHECKLIST.md).

Na rodada de 2026-10-19, o modo guiado acrescentou oito variações locais e parou pelo critério de platô: 44 casos ao todo, 40 concluídos, quatro reversões esperadas, zero resultados novos. O plano externo ficou vazio. Veja [triagem local](EVIDENCE-FILTER.md).

Na rodada `precision-v2`, a escolha de entradas incluiu seis valores próximos a mudanças de preview e quatro sequências de ordem/estado. O conjunto de decisões observadas não aumentou; o caminho da IA ficou vazio. O relatório separa medidas observadas de pontos com alcance ainda desconhecido.
