# VPS de desenvolvimento e análise

Estado verificado em 2026-10-18 para `srv2074743.hstgr.cloud` (Hostinger KVM 4). Este servidor executa o motor localmente; não há API, serviço web ou integração DeepSeek implantados.

## Recursos e acesso

- Ubuntu 26.04.1 LTS, x86_64, 4 vCPU, 16 GiB de RAM e disco de 200 GB.
- SSH na porta 22. A chave pública ED25519 autorizada pelo proprietário foi adicionada via hPanel. Nenhuma chave privada foi copiada para o VPS.
- Usuário `toupera`, sem sudo, dono de `/srv/toupera-agent`. O acesso administrativo existente por `root` não foi removido; a autenticação por senha do SSH também permanece habilitada.
- UFW ativo com política de entrada `deny` e somente OpenSSH permitido. Fail2ban ativo para `sshd` (5 falhas em 10 minutos, bloqueio por 1 hora).
- Atualizações automáticas de segurança já estavam ativas; `apt-daily-upgrade.timer` estava habilitado. Pacotes do sistema foram atualizados em 2026-10-18.
- Swap de 4 GB em `/swapfile`, persistido em `/etc/fstab`.

## Ferramentas instaladas

| Ferramenta | Versão verificada | Local |
| --- | --- | --- |
| Rust / Cargo | 1.93.1 | pacotes Ubuntu |
| `solc` | 0.8.37 | `/usr/local/bin/solc` |
| Slither | 0.11.6 | `/home/toupera/.local/bin/slither` (pipx) |
| Foundry / `forge` | 1.8.5 | `/home/toupera/.foundry/bin/forge` |

O binário `solc` veio de `binaries.soliditylang.org` e foi conferido com SHA-256 `5de843c2c93563cc66425c99a4fb13fdbf32b4c4ae07469480faaf126e14404a` da lista oficial. O instalador Foundry verificou a atestação e os hashes dos binários. Slither foi instalado em ambiente Python isolado com pipx. Os caminhos de Slither e Foundry foram adicionados ao `PATH` de login de `toupera`.

## Projeto e execução

Um snapshot do repositório privado foi enviado por `git archive` a `/srv/toupera-agent`, sem armazenar credenciais do GitHub no VPS. A revisão sincronizada fica registrada em `/srv/toupera-agent/.deployed-commit`. O CLI foi compilado com:

```sh
cd /srv/toupera-agent
cargo build --release --locked -p security-engine
./target/release/security-engine tests/fixtures/VaultCallFlowFixture.sol /usr/local/bin/solc
```

A execução local dessa fixture produziu 4 registros de IR. Slither analisou a mesma fixture com 102 detectores e gerou 3 alertas no JSON; seu código de saída 255 indicou alertas encontrados, e `success` foi `true`. Esses resultados são apenas verificações de funcionamento das ferramentas em uma fixture, não vulnerabilidades confirmadas.

O servidor não contém `.git` nem credenciais para acessar o repositório privado. Atualizações do projeto devem ser enviadas novamente a partir de um checkout autorizado e recompiladas. Não existem serviço `systemd`, endpoint HTTP, banco de dados ou chave da API DeepSeek configurados. O CLI atual só aceita um arquivo Solidity sem imports e um caminho opcional para `solc`; não recebe tarefas de rede nem chama IA.

## Operação segura

Use apenas fontes próprias, forks locais ou programas com autorização explícita. Antes de analisar outro alvo, confirme o escopo. Nunca registre chave da API, senha ou chave SSH privada no repositório, nos argumentos de linha de comando ou nos logs. A integração DeepSeek depende de implementação futura e de um mecanismo de segredo no servidor. Não exponha `anvil` na rede pública.
