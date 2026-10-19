# Reconciliação forense offline do mapa wCOP — 2026-10-19

## Fonte e método

- Recorte principal: trabalho privado `92273047cbf7439c`, Ethereum bloco `26228846`. Conferência independente do mesmo método no trabalho `7d78331aa23a4592`, bloco `26228635`.
- `tools/wcop_forensic_map.py` lê apenas os artefatos já guardados no VPS. Confere hash do runtime entre motor, mapa e duas leituras; confere bloco e contrato; confronta o mapa do compilador com os PCs observados e recompõe as ligações ausentes do grafo estático.
- Os 34 arquivos da entrada do compilador têm conteúdo idêntico às cópias usadas pela cobertura da fonte. O script compara os totais de ramos de `coverage.json` com o LCOV, excluindo os arquivos de teste da mesma forma que o coletor original.
- Saída detalhada e hashes das entradas: `private-output/site-jobs/<job-id>/forensic-map.json`, modo `0600`, propriedade de `toupera`. O código da ferramenta e este relatório não incluem os artefatos privados.

## Resultado observado

| Medida | Resultado |
| --- | ---: |
| PCs observados no runtime | 3.014 |
| PCs observados com local de fonte ou arquivo interno | 2.478 |
| PCs observados sem local | 536 |
| PCs sem local cujo identificador do compilador é `-1` | 536 |
| PCs observados após o fim do mapa, PC 8499 | 0 |
| PCs desmontados após o fim do mapa | 37 |
| Ligações observadas ausentes do grafo estático | 74 |
| Dessas ligações, blocos de origem terminados em `JUMP` | 74 |
| Marca de saída de função `o` / salto comum `-` no opcode final | 68 / 6 |
| Ligações observadas com origem terminada em `JUMPI` | 143 |
| Dessas ligações condicionais ausentes do grafo estático | 0 |
| Dessas ligações cujo `JUMPI` está em linha com registro LCOV | 38 |
| Ligações fonte → fonte | 54 |
| Ligações código gerado → código gerado | 8 |
| Ligações código gerado → fonte | 12 |
| Registros de ramo LCOV da fonte recompilada | 38 vistos / 213 |

As 74 ligações têm 39 origens e 65 destinos distintos. Cada nó é um ID de entrada de bloco; o opcode final do bloco de origem foi recuperado pelo motor. Todos os 74 blocos de origem terminam em `JUMP` incondicional. No mapa de fonte do compilador, 68 desses opcodes têm a marca `o` (saída de função) e seis a marca `-` (salto comum), conforme a [semântica documentada pelo Solidity](https://docs.soliditylang.org/en/v0.8.30/internals/source_mappings.html). Assim, o grupo fora do grafo estático é predominantemente de saltos de retorno; não há base para chamá-lo de ramo condicional anômalo ou falha.

No conjunto de 344 ligações observadas entre blocos, 143 têm bloco de origem terminado em `JUMPI`, 188 em `JUMP` e 13 em outros opcodes. As 143 primeiras já constam no grafo estático. Em 38 delas, o `JUMPI` final fica numa linha que possui registro de ramo LCOV. **Esse 38 é uma contagem de ligações e coincide numericamente, por acaso, com os 38 registros de ramo atingidos pelo LCOV; não representa a mesma medição.**

O LCOV contém 179 linhas com registros de ramo. Em 28 dessas linhas também há pelo menos um PC observado com local de fonte; 7 linhas de ramo aparecem como origem ou destino das ligações acima, e 10 ligações tocam uma dessas linhas. Essa é **apenas correspondência por arquivo e linha** entre duas medições: o LCOV veio da fonte recompilada, enquanto os PCs vieram do runtime fixado no bloco. Ela não prova que um ramo específico do contrato publicado foi executado.

Os dois trabalhos produziram a mesma classificação e o mesmo runtime. Isso reproduz a leitura offline; não amplia a cobertura nem confirma comportamento incorreto. O vínculo entre fonte recompilada e bytecode permanece `confere sem campos imutáveis`, com a ressalva já registrada. O filtro final continua `local_review`, sem diferença local repetida.

## Limites e próxima verificação

- `file_id = -1` é a causa registrada pelo mapa do compilador para os 536 PCs observados sem local. Não atribuir uma linha por proximidade sem prova adicional.
- O bloco de origem permite recuperar seu opcode final e a marca de salto do compilador. `trace.json` e `adaptive-trace.json` guardam contagens por caso e ligações agregadas, mas não a pilha, o destino calculado ou o estado imediatamente anterior a cada passagem; os logs locais do Anvil desta rodada estão vazios. Para ligar uma passagem a um ramo específico, a próxima coleta deve preservar por caso o PC e opcode do salto, profundidade de chamada, condição e destino calculado, junto com a identidade do runtime e do bloco. Só então comparar com o ramo da fonte recompilada, mantendo as duas medições separadas.
- A linha de versões continua limitada pelo estado histórico indisponível no RPC. Nenhuma consulta histórica nova foi feita nesta etapa.

Validação no VPS, sem nova rodada do contrato: `python3 tools/wcop_forensic_map.py 92273047cbf7439c` e o mesmo comando para `7d78331aa23a4592`, ambos como `toupera`; as verificações de identidade, fontes e totais passaram. SHA-256 do script: `0ab21616db6739240cc0c5ada9391d5bf8cd647ffb51f864a370740eeca7ff2f`. SHA-256 do JSON do trabalho principal: `64c639021ea4ee82edf0fc0d219437c4b48358f79610b950383fb425090fbaaa`.
