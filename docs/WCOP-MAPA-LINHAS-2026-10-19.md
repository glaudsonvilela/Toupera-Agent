# wCOP — mapa de linhas e sequência de permissões

## Mudança no processo

- A recompilação passa a guardar o mapa de posições do compilador. O leitor `tools/wcop_path_map.py` liga posições vistas nas duas leituras locais a arquivo e linha, após conferir código, configuração, bloco e ressalva dos campos fixos.
- O trabalho do painel executa essa leitura automaticamente e inclui `path-map.json` no manifesto privado. O verificador Rust confere hash, identidade, relação da fonte e tamanho do mapa antes de apresentar a observação.
- A leitura de permissões refaz a sequência dos registros de concessão e retirada e compara o estado final de cada conta vista com `hasRole` no bloco fixado. `complete_account_discovery` continua falso.

## Aplicação retrospectiva à rodada 7011fcd042db41e5

- Bloco Ethereum: `26228465`; endereço: `0x8a1d45e102e886510e891d2ec656a708991e2d76`.
- A fonte recompilada confere com o runtime publicado após mascarar três campos imutáveis. Esta ressalva permanece.
- Posições observadas nas duas leituras: **3048/5377** posições desmontadas.
- Posições com local de fonte atribuído: **3168/5377**; entre as observadas, **1945/3048**. As demais não recebem linha confiável do mapa.
- O mapa do compilador termina na posição `8499`; há **37** posições desmontadas depois desse ponto, nenhuma observada. Essa região contém dados embutidos e não deve ser contada automaticamente como caminho executável.
- Ligações observadas fora do mapa estático, reunindo as duas leituras: **74**. A leitura inicial sozinha tinha 61; os números não representam diferenças de estado do contrato.
- Organização para revisão: 7 ligações passam pelo arquivo principal, 25 por arquivos de permissões, 15 por arquivos de assinatura/domínio e 27 por outros arquivos.
- Sequência de permissões: **15** registros para **5** contas candidatas. A reaplicação offline desses registros coincide com as contas ativas já consultadas no bloco para os quatro papéis conhecidos: **sim**. A nova comparação por RPC valerá para os próximos trabalhos.

## Limites

- Local de fonte indica origem atribuída pelo compilador; não comprova que uma condição seja alcançável em outros estados.
- A razão 3048/5377 usa posições desmontadas, incluindo locais sem mapa; não é cobertura total do contrato.
- A revisão retrospectiva adicionou `path-map.json` ao diretório privado do trabalho anterior. Ela não alterou o manifesto nem a decisão daquele trabalho já concluído. A nova ligação ao verificador passa a valer para trabalhos futuros.
- Registros completos desde a criação ajudam a reconstruir permissões, mas mudanças de implementação ou gravações sem registro ainda podem deixar contas fora da lista.
- Não há diferença de estado confirmada nesta etapa.

## Próximas prioridades

1. Separar dados embutidos do denominador de posições de execução e medir entradas alcançadas por função.
2. Agrupar as 74 ligações por origem e resultado de chamada para melhorar o mapa estático.
3. Ampliar combinações de estado com orçamento controlado e observar se surgem linhas ou saídas novas.
