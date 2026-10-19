# Cenários locais no VPS

O comando Rust `--local-scenarios <forensic-report.json> <fork-rpc-url> <anvil-bin>` usa um relatório `forensic-map-v1` já produzido. Antes de qualquer operação, ele confere chain ID, hash do bloco fixado, hash do runtime do vault e endereço retornado por `asset()`. Abre um processo Anvil filho em `127.0.0.1`, com fork no bloco fixado, e encerra esse processo ao terminar. Transações de cenário são encaminhadas **somente** a esse processo local.

O ator é o owner registrado no relatório. O fork local dá a esse ator saldo de gas e aprovação do asset, sem alterar a rede de origem. Para `deposit`, `mint`, `withdraw` e `redeem`, o motor seleciona entradas pequenas e uma entrada proporcional ao saldo ou à precisão do token. Cada caso isolado começa no mesmo snapshot. Uma sequência adicional faz depósito seguido do resgate das shares recebidas, sem restaurar o estado entre as duas etapas.

O JSON `local-scenarios-v1` registra preview, recibo, gas, estado antes/depois, diferenças de assets/shares, reversões e pontos de desvio (`JUMPI`) observados pelo rastro local. Diferenças são **sinais para revisão**, não achados confirmados. A contagem de pontos usa pares `(profundidade, PC)` de todas as chamadas na operação, inclusive ao asset; não é cobertura por função nem percentual de ramos do vault. Reversões podem ser esperadas para entradas pequenas. O motor não deduz segurança quando todos os casos passam.

Este comando é separado do M7. O replay histórico registra transações publicadas; estes cenários executam novas operações apenas no fork local. A etapa seguinte é vincular eventuais diferenças ao mapa/quarentena e calibrar a seleção M4 antes do percurso positivo M5–M8.
