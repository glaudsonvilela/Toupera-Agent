# Rodada integrada wCOP — 7d78331aa23a4592

- Endereço: `0x8a1d45e102e886510e891d2ec656a708991e2d76` na Ethereum; bloco `26228635`; hash `0x2056f2e6cf4acf780226f6c025e4add2fa2010120b0ed6201418491172c93467`.
- Implementação: `0x6f6206fac28d9f60d221a63aae5fb5c3a9ea6652`; vínculo de fonte `masked_immutables`, com ressalva dos campos imutáveis.
- Os três arquivos do processo foram gerados no VPS e ficam privados. SHA-256:
  - `smart.md`: `522b351178801d075b28f796f98840e87ea94ca1884858d59d0f952233afc485`.
  - `result_smart.md`: `6a9d233c7d898f3ab9fc2bccf2c4a9de690128c8cc2f24e3664f3a6397cb2da9`.
  - `end_smart_result.md`: `d038d1020a47b2138adce5033a1363e6b8f43a8d0e0abe7b97fe02ab4fa6e81c`.

## O que a rodada observou

- Histórico: 236 registros em 5 páginas; emissão menos retirada confere com a oferta no bloco.
- Permissões: 15 registros, 5 contas vistas, sequência final compatível com as consultas: sim. Descoberta de todas as contas: não comprovada.
- Fonte recompilada: 38/213 ramos medidos no conjunto de fonte e bibliotecas. Essa contagem não inclui o ciclo posterior no runtime.
- Runtime: 3014/5377 posições desmontadas observadas; 2478 posições observadas receberam local do compilador, incluindo 541 de arquivos internos. 536 ficaram sem local atribuído.
- O mapa de fonte termina antes de 37 posições desmontadas finais; nenhuma delas foi observada. O total desmontado não é denominador de alcance comprovado.
- Rust: 33 entradas ordenadas e 74 ligações observadas fora do mapa estático. O ciclo fez 38 escolhas, 20 com preparação local, e acrescentou 544 posições, 48 ligações e 37 blocos à primeira leitura. Parou por `plateau`.
- Diferenças nas verificações de estado: 0. O leitor Rust dos caminhos registrou 0 erros de vínculo.
- Linha de versões: 1 registro de troca; 1 bloco antigo sem estado disponibilizado pelo provedor; versão atual e último registro concordam.
- Filtro final: `local_review`; itens liberados para contexto adicional: 0; chamada externa: não.

## Ferramentas registradas

| Ferramenta | Estado |
|---|---|
| Slither | executado |
| Mythril | executado |
| Securify2 | não aplicável |
| Manticore | não aplicável |
| Foundry | executado |
| Echidna | executado |
| Medusa | executado |
| Certora | indisponível |
| Halmos | concluído |
| Kontrol | pendente |

## Leitura correta do resultado

- Esta rodada não confirmou uma diferença de comportamento do contrato. A ordem calculada pelo Rust orienta leituras futuras; não é um veredito.
- A medição anterior observou 3048 posições após 40 escolhas; esta observou 3014 após 38. Blocos e escolhas diferem. A comparação entre rodadas não mede melhora ou piora de cobertura completa.
- O estado do bloco antigo não foi fornecido pelo RPC, por isso a linha de versões é limitada.
- Ferramentas marcadas como não aplicáveis, indisponíveis ou pendentes não foram contadas como executadas.
- As operações com mudança de estado ocorreram apenas na cópia local. Nenhuma transação foi enviada à rede pública.
