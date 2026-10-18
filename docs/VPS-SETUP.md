# VPS de desenvolvimento e análise

Estado verificado em 2026-10-18 para `srv2074743.hstgr.cloud` (Hostinger KVM 4). Este servidor executa o motor localmente; não há API, serviço web ou integração DeepSeek implantados.

## Recursos e acesso

- Ubuntu 26.04.1 LTS, x86_64, 4 vCPU, 16 GiB de RAM e disco de 200 GB.
- SSH na porta 22. A chave pública ED25519 autorizada pelo proprietário foi adicionada via hPanel. Nenhuma chave privada foi copiada para o VPS.
- Usuário `toupera`, sem sudo, dono de `/srv/toupera-agent`. O acesso administrativo existente por `root` não foi removido; a autenticação por senha do SSH também permanece habilitada.
- UFW ativo com política de entrada `deny` e somente OpenSSH permitido. Fail2ban ativo para `sshd` (5 falhas em 10 minutos, bloqueio por 1 hora).
- Atualizações automáticas de segurança já estavam ativas; `apt-daily-upgrade.timer` estava habilitado. Pacotes do sistema foram atualizados em 2026-10-18.
- O VPS foi reiniciado após as atualizações; SSH, Fail2ban, UFW, swap e as ferramentas foram verificados novamente. O marcador `reboot-required` desapareceu.
- Swap de 16 GB em `/swapfile`, persistido em `/etc/fstab`, para acomodar instalações e provas mais pesadas como Kontrol.

## Ferramentas instaladas

| Ferramenta | Versão verificada | Local |
| --- | --- | --- |
| Rust / Cargo | 1.93.1 | pacotes Ubuntu |
| `solc` | 0.8.37 | `/usr/local/bin/solc` |
| Slither | 0.11.6 | `/home/toupera/.local/bin/slither` (pipx) |
| Foundry / `forge` | 1.8.5 | `/home/toupera/.foundry/bin/forge` |
| Mythril / `myth` | 0.24.8 | `/home/toupera/.local/bin/myth` (uv, Python 3.10) |
| Securify2 | commit `1913dfe` | imagem Podman rootless `localhost/toupera-securify2:legacy` |
| Echidna | 2.3.3 | `/usr/local/bin/echidna` |
| Medusa | 1.5.1 | `/usr/local/bin/medusa` |
| Halmos | 0.3.3 | `/home/toupera/.local/bin/halmos` (uv, Python 3.12) |
| Certora CLI / `certoraRun` | 8.20.2 | `/home/toupera/.local/bin/certoraRun` (uv, Python 3.12) |
| Manticore | 0.3.7 | imagem Podman rootless `docker.io/trailofbits/manticore:latest` |
| Kontrol | 1.0.255 | imagem Podman rootless `docker.io/runtimeverificationinc/kontrol:ubuntu-jammy-1.0.255` |

O binário `solc` veio de `binaries.soliditylang.org` e foi conferido com SHA-256 `5de843c2c93563cc66425c99a4fb13fdbf32b4c4ae07469480faaf126e14404a` da lista oficial. O instalador Foundry verificou a atestação e os hashes dos binários. Slither foi instalado em ambiente Python isolado com pipx. Os caminhos de Slither e Foundry foram adicionados ao `PATH` de login de `toupera`.

Mythril, Halmos e Certora usam ambientes Python isolados; Mythril precisou de `setuptools==80.9.0` para fornecer `pkg_resources`. Java 21 foi instalado para o CLI Certora. Echidna e Medusa vieram dos arquivos oficiais de release. Podman roda como usuário `toupera`, sem daemon Docker nem portas publicadas. Manticore está arquivado e Kontrol usa uma tag específica porque a imagem do fornecedor não publica `latest`.

Securify2 foi compilado a partir do commit indicado na tabela, com o `solc` 0.5.12 padrão de sua receita. Apenas na cópia de instalação em `/srv/toupera-tools/securify2`, `graphviz` foi fixado em `0.19.2` em `setup.py` e `requirements.txt`, pois a versão atual não suporta o Python 3.7 da imagem Ubuntu 18.04. A imagem foi construída e o CLI respondeu a `--help`; isso não estabelece compatibilidade com Solidity moderna. Manticore respondeu `Manticore 0.3.7` e Kontrol respondeu `Kontrol version: 1.0.255` dentro dos respectivos contêineres.

O CLI Certora está instalado, mas **não** há `CERTORAKEY` configurada e nenhuma prova foi enviada ao serviço externo. Instalação de ferramenta não significa integração ao Toupera Agent nem validação de achados em contratos reais. Manticore e Securify2 usam dependências antigas e podem não interpretar corretamente Solidity/EVM atuais.

Para abrir os contêineres como `toupera` a partir de uma sessão administrativa, use `runuser -l toupera -c 'XDG_RUNTIME_DIR=/run/user/1000 podman run --rm --network=none IMAGEM COMANDO'`. Monte apenas um diretório de trabalho autorizado quando for analisar contratos. Os 10 executores da estratégia estão disponíveis, mas o CLI Rust ainda não os orquestra automaticamente.

## Projeto e execução

Um snapshot do repositório privado foi enviado por `git archive` a `/srv/toupera-agent`, sem armazenar credenciais do GitHub no VPS. A revisão sincronizada fica registrada em `/srv/toupera-agent/.deployed-commit`. O CLI foi compilado com:

```sh
cd /srv/toupera-agent
cargo build --release --locked -p security-engine
./target/release/security-engine tests/fixtures/VaultCallFlowFixture.sol /usr/local/bin/solc
```

A execução local dessa fixture produziu 4 registros de IR. Slither analisou a mesma fixture com 102 detectores e gerou 3 alertas no JSON; seu código de saída 255 indicou alertas encontrados, e `success` foi `true`. Esses resultados são apenas verificações de funcionamento das ferramentas em uma fixture, não vulnerabilidades confirmadas.

O servidor não contém `.git` nem credenciais para acessar o repositório privado. Atualizações do projeto devem ser enviadas novamente a partir de um checkout autorizado e recompiladas. O recorte M3 de runtime legado, incluindo fluxo abstrato de stack/memória, segunda leitura local com EVMole 0.9.4 e comparação opcional com source, foi sincronizado e compilado em release; a revisão implantada fica em `.deployed-commit`. Não existem serviço `systemd`, endpoint HTTP, banco de dados ou chave da API DeepSeek configurados. O CLI aceita um arquivo Solidity sem imports e um caminho opcional para `solc`, ou `--bytecode <runtime.hex>`; a comparação com saída local do compilador usa `--compare-bytecode`. Ele não recebe tarefas de rede nem chama IA.

## Companions de engenharia reversa

- Heimdall-rs foi instalado em `/usr/local/bin/heimdall` a partir do asset oficial `heimdall-linux-amd64` da release `0.9.3`. O próprio binário informa `heimdall 0.9.2` em `--version`; registrar **ambos** os identificadores até o upstream esclarecer a divergência. SHA-256 do asset e binário instalado: `8d58c97f56d584d5d0737d0d0895fc8c68b7b8b2b680f2a7842245789f0b4868`.
- A imagem oficial `ghcr.io/nevillegrech/gigahorse-toolchain` está no Podman rootless de `toupera`, digest `sha256:f676ca8aaf88acd47be27ed1967acddc9c99acdd041b34e79472cfb028910743`. O wrapper usa esse digest por padrão, com `--network none`, montagem apenas do bytecode local, diretório de saída e cache. `podman run ... --help` abriu; nenhum contrato foi decompilado nesta instalação.
- O CLI Rust oferece `--bytecode-companions` para importar ABI Heimdall e TAC Gigahorse; `scripts/run-bytecode-companions.sh <runtime.hex> <diretório-novo>` executa os dois e salva `analysis.json` com artefatos intermediários. Defina `TOUPERA_SECURITY_ENGINE=/srv/toupera-agent/target/release/security-engine` se o binário não estiver no `PATH`. Leia `docs/BYTECODE-RECONSTRUCTION.md`. Isso acrescenta evidência tentativa; não ativa serviço web nem IA.

## Análise econômica M4

O CLI `--economic-candidates <source.sol> [solc-path] [slither.json]` emite candidatos `UNVERIFIED` da IR source-first e importa opcionalmente sinais do Slither. `scripts/run-economic-candidates.sh <source.sol> <diretório-novo>` executa Slither localmente, guarda uma cópia do source, versões, JSON Slither e `candidates.json`. Defina `TOUPERA_SECURITY_ENGINE=/srv/toupera-agent/target/release/security-engine` quando necessário. O wrapper não foi executado em fixture ou alvo nesta etapa; testes e calibração de falsos positivos ficam para depois da implementação completa, por solicitação do usuário. Leia `docs/ECONOMIC-ANALYSIS.md`.

A revisão de implementação `d2ed831` foi sincronizada e compilada em release em 2026-10-18. O código fica disponível para execução futura, sem resultados de teste de detecção nesta etapa.

## Pacote de contexto M5

`--context-package <source.sol> <private-cache-dir> <solc-path> [--slither <json>] [--bytecode <runtime.hex>]` gera slices source-backed e guarda o pacote reduzido em cache local privado. Use `umask 077` antes de redirecionar stdout para um arquivo: a saída pode conter trechos de source privado. O diretório de cache deve ter modo `0700`; entradas usam `0600`. O CLI não chama DeepSeek nem outro serviço externo. Leia `docs/CONTEXT-PACKAGE.md`. Nenhuma fixture foi executada para este modo antes da validação geral solicitada pelo usuário.

## Operação segura

Use apenas fontes próprias, forks locais ou programas com autorização explícita. Antes de analisar outro alvo, confirme o escopo. Nunca registre chave da API, senha ou chave SSH privada no repositório, nos argumentos de linha de comando ou nos logs. A integração DeepSeek depende de implementação futura e de um mecanismo de segredo no servidor. Não exponha `anvil` na rede pública.
