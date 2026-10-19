# Guardião de evidências — primeira integração

Data: 2026-10-19. Escopo: painel privado do wCOP na Ethereum. Implementação e conferências executadas exclusivamente no VPS. Complementa o desenho em ARQUITETURA-CONFIANCA-2026-10-19.md.

## O que entrou no fluxo

1. A API mantém um endereço permitido e fixa número/hash de bloco. O trabalhador reconfirma o hash ao terminar a leitura.
2. O manifesto privado job-manifest.json registra endereço, bloco, palavra do slot EIP-1967, implementação, hashes e tamanhos dos artefatos, além dos hashes dos binários Rust e solc.
3. O compilador solicita AST de cada fonte. A ingestão rejeita caminho, arquivo repetido e volume fora do limite. O Rust confere IDs, faixas de bytes, caminhos e a igualdade entre fontes compiladas e cópias usadas pelas ferramentas.
4. O Rust confere o código do proxy e da implementação com seus mapas, o slot com o endereço da implementação e o runtime recompilado com os bytes do bloco. Quando a diferença cabe somente nos campos imutáveis declarados, marca ressalva e não afirma igualdade dos valores.
5. O Rust emite decision.json com um estado por afirmação. O relatório do painel mostra essa decisão antes dos dados coletados. Caso o guardião não conclua, o relatório registra estado inconclusivo; relações de fonte ficam suspensas.
6. Sem fonte ou AST, a leitura direta do bytecode continua, com estado parcial. Respostas das ferramentas ficam como sinais brutos; as propriedades locais são descritas somente sob as premissas do harness.

Código: crates/security-engine/src/assurance.rs e CLI --wcop-guard; integração em tools/wcop_scope.py e tools/site_jobs.py. O relatório de um trabalho fica em end_smart_result.md; os JSONs completos permanecem em private-output/site-jobs no VPS.

## Estados produzidos

- OBSERVADO: medição ligada aos artefatos fixados e ao alcance declarado.
- RELACIONADO_COM_RESSALVA: fonte e runtime só conferem após mascarar posições imutáveis declaradas.
- HIPOTESE: sinal de ferramenta sem reprodução e revisão.
- INCONCLUSIVO: entrada, vínculo ou propriedade insuficiente.

A decisão global é escopo_vinculado, bytecode_parcial ou inconclusivo. Nenhum destes estados confirma um problema no contrato nem prova ausência de problemas.

## Conferência no VPS

O trabalho privado 3932c3f9ca4e4faf concluiu no bloco 26228121. O guardião confirmou identidade dos artefatos; a AST foi emitida para 34 fontes e o vínculo de fonte recebeu a ressalva de três campos imutáveis. Bytecode, histórico e caminhos locais foram observados dentro do bloco e dos casos executados; as propriedades locais permaneceram explicitamente limitadas. O relatório final foi exibido pela API do painel.

Em cópia privada de um trabalho anterior, uma faixa AST adulterada rebaixou o vínculo para diverged/bytecode_parcial; restaurar o arquivo retornou masked_immutables/escopo_vinculado. Outra cópia sem fonte preservou a leitura do bytecode e marcou a fonte como unavailable. Estas conferências ocorreram só no VPS e não alteraram o trabalho publicado.

## Limites honestos

- Comparar o resultado de solc com o runtime não é uma prova de preservação semântica do compilador Solidity, como a prova específica de CompCert para C.
- Verificar a estrutura da AST não prova a correção do contrato ou do próprio parser.
- Propriedades Halmos e campanhas Foundry/Echidna/Medusa cobrem apenas os estados, limites e premissas dos harnesses locais.
- O guardião classifica evidências e pode suspender conclusões do painel. Ele não bloqueia transações na rede nem altera bytecode implantado.
- Processos externos já têm tempo limitado e logs privados, mas ainda rodam sob o mesmo usuário de serviço; o tamanho dos logs não é limitado. Separação por permissões e isolamento adicional continuam como melhoria específica.
- O guardião já qualifica o relatório do painel; o pacote M8 de entrega ainda não consome automaticamente decision.json. Nenhum envio a programa externo ocorre pelo painel.

## Próximas verificações de engenharia

- Vincular o pacote M8 ao decision.json antes de permitir uma conclusão revisada.
- Dar a cada propriedade um identificador, condições de entrada e artefatos locais verificáveis no guardião.
- Isolar compilação e ferramentas em ambiente com acesso mínimo aos diretórios do trabalho.
- Medir rejeições justificadas e resultados inconsistentes em outros contratos autorizados, sem transformar cobertura local em uma porcentagem global.
