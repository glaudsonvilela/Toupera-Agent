# Arquitetura de confiança do motor

Estado: proposta de arquitetura, anterior à implementação. Escopo inicial: painel do wCOP na Ethereum, bloco fixado, execução local no VPS e relatório privado. Esta etapa documenta interfaces e critérios; não altera o fluxo em produção.

## Objetivo

Fazer cada conclusão depender de artefatos identificáveis, limites declarados e uma checagem independente. O motor continua a examinar bytecode mesmo sem fonte. Fonte, AST, saída das ferramentas e texto da IA são entradas não confiáveis até que a camada correspondente as valide.

## Fluxo proposto

    Endereço permitido + bloco fixado
             |
             v
    [A] Coleta e identidade
        chain ID, hash do bloco, proxy, implementação, hashes de runtime
             |
             +----------------------+
             |                      |
             v                      v
    [B] Fonte disponível?        [C] Bytecode direto
        bundle limitado              CFG, seletores, efeitos,
        solc em processo             slots, PCs, lacunas
        AST conferida                sem nomes inventados
        fonte ↔ runtime                  |
             |                      |
             +----------+-----------+
                        v
    [D] Núcleo Rust de relações e hipóteses
        proveniência por arquivo/faixa ou PC
                        |
                        v
    [E] Executores locais isolados
        Slither, Foundry, Echidna, Medusa, Halmos etc.
        resultados brutos com versão, escopo e limites
                        |
                        v
    [F] Guardião de evidências em Rust
        valida identidade, proveniência, consistência e suficiência
          |              |                 |
          v              v                 v
       sustenta       inconclusivo       incompatível
          |              |                 |
          +--------------+-----------------+
                         v
    [G] Relatório + painel + revisão humana
        IA opcional só recebe recorte aprovado pelo guardião

## Fronteiras de confiança

| Entrada | Tratamento | Condição para uso |
| --- | --- | --- |
| Endereço do painel | Lista permitida, chain ID e bloco fixado | Mesmo alvo em todos os artefatos |
| RPC e explorador | Dados externos; registrar origem e hash | Reconfirmar bloco, código e slot do proxy |
| Arquivos Solidity | Limite de tamanho, caminhos relativos, nomes únicos, hash por arquivo | Compilação reproduzível e vínculo com runtime |
| AST e JSON do compilador | Esquema, profundidade, IDs e faixas de bytes verificados | Nada fora da fonte correspondente; erro do compilador interrompe trilha de fonte |
| Bytecode EVM | Normalização explícita e hash próprio | PC e seletor referidos devem existir no código fixado |
| Ferramentas locais | Processos separados, tempo e saída limitados | Status, versão e propriedade examinada registrados |
| IA | Só recorte mínimo, sem autoridade para aprovar | Toda hipótese volta ao guardião e a um ensaio local |
| Relatório | Usa decisão estruturada, não texto bruto de ferramentas | Toda afirmação aponta para evidência e ressalva |

O guardião decide sobre a validade da conclusão produzida pelo sistema. Ele não controla transações de contratos de terceiros na rede. A execução que altera estado permanece apenas na cópia Anvil local.

## Interfaces entre componentes

O trabalho privado mantém os arquivos atuais: smart.md, runtime.hex, implementation.hex, compiler-input.json, compiler-output.json, engine-impl.json, scope.json, trace-reconcile.json e resultados das ferramentas. A implementação adicionará:

1. job-manifest.json: chain ID, endereço, bloco/hash, proxy, implementação, hashes de runtime e versões das ferramentas.
2. parse-report.json: caminhos de fonte, hashes, AST presente, faixas válidas, erros e limites de processamento.
3. identity-report.json: modo de vínculo exato, vínculo após mascarar imutáveis, divergência ou fonte ausente. Valores imutáveis mascarados ficam explicitamente sem conferência de igualdade.
4. evidence-bundle.json: para cada afirmação, artefatos, hash, função ou PC, propriedade testada, resultado, limites e origem.
5. decision.json: estado por afirmação, razões legíveis e condição pendente. O painel e a IA consomem esta decisão, sem inferir status de logs livres.

Todos os arquivos ficam em private-output/site-jobs/<id> com permissões privadas. O documento final do painel mostra um resumo, com links internos apenas quando houver controle de acesso.

## Regras do guardião

- Identidade diferente, arquivo alterado, AST inválida ou runtime divergente: suspender conclusões baseadas em fonte; manter apenas leitura direta do bytecode e indicar a causa.
- Compilação equivalente após mascarar imutáveis: permitir mapa estrutural de fonte, mas preservar a lacuna dos valores imutáveis.
- Seletor ou PC sem correspondência: marcar a relação como não resolvida; não ligar automaticamente a uma função de fonte.
- Saída de ferramenta sem propriedade, versão, limite ou resultado estruturado: conservar como sinal bruto.
- Propriedade local que passou em N casos: descrever exatamente esses casos; não generalizar para todos os estados.
- Contraexemplo reproduzido duas vezes com identidade e condições iguais: oferecer revisão humana, sem confirmação automática.
- Ausência de fonte: usar a trilha de bytecode com proveniência em PCs; não reconstruir nomes ou intenções como fatos.
- IA: hipótese sempre fica pendente de reprodução e revisão; resposta de modelo nunca eleva sozinha o estado.

Estados propostos por afirmação: OBSERVADO, RELACIONADO_COM_RESSALVA, HIPOTESE, INCONCLUSIVO, REPRODUZIDO_PARA_REVISAO e REVISAO_HUMANA. A decisão inclui separadamente identidade, alcance e fundamento; não cria uma porcentagem global de certeza.

## Adaptação dos conceitos citados

| Referência | Adaptação no projeto | Limite |
| --- | --- | --- |
| HACMS e partições | Separar coleta, compilação, motor, executores, guardião e publicação por contratos de dados | Isolamento de processos e invariantes do nosso código ainda precisarão de implementação e validação |
| seL4 | Escrever premissas de cada propriedade e conferir estados permitidos no guardião | A prova de seL4 não transfere uma prova matemática ao nosso motor |
| CompCert | Reproduzir o build Solidity, guardar entradas/versão e comparar código implantado | Comparação de bytes não prova preservação semântica do compilador Solidity |
| SafeDocs | Validar arquivo, AST, JSON e limites antes de alimentar o motor | Um AST bem formado não prova que o código é correto |
| AMP | Manter análise bytecode-first quando a fonte faltar; cruzar com fonte quando houver | Não há micropatch de bytecode implantado nem alteração de contrato de terceiros |

## Ordem de implementação

1. Identidade e ingestão: manifesto, limites, hashes, bloco fixado, tratamento de arquivos repetidos e caminhos.
2. Validação de fonte/AST: saída AST do solc e verificador Rust de faixas/IDs; testes de entradas inválidas.
3. Vínculo fonte ↔ runtime: estados exatos e mascarados, premissas de imutáveis e fallback automático ao bytecode.
4. Guardião Rust: regras por afirmação, vínculos entre evidências, motivo de rejeição e JSON de decisão.
5. Adaptadores e painel: ferramentas entregam evidência estruturada; relatório e terminal exibem o estado do guardião.
6. Propriedades e revisão: catálogo de propriedades com alcance explícito; ligar Foundry/Echidna/Medusa/Halmos sem ampliar conclusões além das premissas.
7. Medição no wCOP: medir rejeições justificadas, caminhos preservados, falsos vínculos evitados e custo, antes de ampliar a outros contratos autorizados.

## Critérios de aceite da primeira entrega

- Mesmo job produz o mesmo manifesto e a mesma decisão para os mesmos arquivos fixados.
- Trocar um byte de fonte, runtime, AST ou artefato de ferramenta muda o hash e invalida o vínculo correspondente.
- Falta de fonte ou falha do solc não interrompe a análise de bytecode.
- Divergência entre fonte e runtime bloqueia toda afirmação de equivalência.
- Prova limitada é rotulada com propriedade e premissas; cobertura local é mostrada separada do runtime publicado.
- O painel nunca mostra confirmação automática de falha; o relatório mantém a trilha para revisão.
- Nenhuma chave, log privado ou resultado bruto sai para o Git.

## Estado atual e próxima lacuna

Já existem bloco e implementação fixados, comparação de 34 fontes com runtime mascarando três campos imutáveis, mapa Rust do bytecode, trilha local no Anvil e resultados das ferramentas. Ainda não existe guardião único por afirmação, manifesto completo de identidade, validação profunda da AST na fronteira ou separação executável de permissões por componente. Esta arquitetura é o roteiro para essas mudanças, não a declaração de que já foram feitas.

## Referências primárias

- DARPA HACMS: https://www.darpa.mil/news/resources/case-studies/hacms
- seL4, provas e premissas: https://sel4.systems/Verification/proofs.html ; https://sel4.systems/Verification/assumptions.html
- CompCert, alcance da prova: https://compcert.org/man/manual001.html
- DARPA SafeDocs: https://www.darpa.mil/research/programs/safe-documents
- DARPA AMP: https://www.darpa.mil/research/programs/assured-micropatching

## Implementação inicial

A primeira integração e suas lacunas estão registradas em [Guardião de evidências](GUARDIAO-EVIDENCIAS-2026-10-19.md).
