# Toupeira Agente — homepage

Homepage pública para `https://toupeiraagente.tech/`. Marca pública: **Toupeira Agente**, conforme pedido do usuário. O nome histórico do repositório e dos diretórios do motor foi preservado.

HTML, CSS e JavaScript sem dependências de execução. Imagem cinematográfica, fumaça procedural em Canvas, partículas, feixe de luz, parallax discreto, transições de entrada e navegação por etapas. O controle de pausa e `prefers-reduced-motion` reduzem movimento; a animação Canvas pausa fora da tela e com a aba oculta. As abas aceitam setas, Home e End. As fontes são servidas localmente.

Os botões navegam pelas seções. A homepage apresenta a tecnologia e não oferece envio público de contratos ou resultados inventados. O motor, seu RPC e os relatórios privados não são expostos pelo site.

## Arquivos e imagem

- `index.html`, `style.css`, `app.js`: interface pública.
- `assets/hero.png`: fundo editado com a ferramenta nativa ImageGen a partir da referência enviada pelo usuário.
- `assets/hero.webp`: mesma imagem com compressão WebP para carregamento, aproximadamente 169 KiB.
- `assets/space-grotesk.woff2`: fonte Space Grotesk, com licença em `assets/SPACE-GROTESK-LICENSE.txt`.
- `deploy/Caddyfile`: configuração do servidor; não copiar para a raiz pública.

Prompt final usado no ImageGen:

> Use case: precise-object-edit. Asset type: full-bleed cinematic website hero background, wide 16:9 landscape. Edit this image: remove ALL typography, lettering, logos, navigation, labels, interface, input field, button, icons, HUD lines and borders, including the large TOUPERA word. Reconstruct the scene naturally behind the removed UI. Preserve the exact white ceramic futuristic hooded robot with deep cobalt blue glass face, its head high in the upper center-right of frame, textured hood, dissolving fragments leftward, white volumetric clouds and smoke, blue highlights, luminous pale gray sky upper half, deep navy smoky lower third. The robot should remain clearly identifiable, majestic, exquisitely detailed. Leave the bottom 35% atmospheric dark navy fog for future HTML typography and buttons. No text anywhere, no watermark. Premium photoreal cinematic 3D, polished and immersive.

## Publicação

Servidor Hostinger `177.7.53.155`, Caddy com HTTPS automático. Apenas `index.html`, `style.css`, `app.js`, `robots.txt`, `sitemap.xml` e `assets/` entram no diretório público. Cada publicação usa uma pasta em `/var/www/toupeiraagente/releases/`; o link `/var/www/toupeiraagente/current` seleciona a versão. A revisão está em `/var/www/toupeiraagente/.deployed-commit`, fora da raiz pública. O host `www` redireciona para o domínio principal.

O serviço web usa o usuário `caddy`; o motor continua sob `toupera`. As únicas portas adicionais necessárias à publicação são TCP 80 e 443. O endpoint administrativo do Caddy fica em loopback.

## Conferência desta versão

Antes da publicação: sintaxe JavaScript, resposta HTTP local, aparência desktop e mobile (390 × 844), navegação das abas por clique e teclado, pausa dos efeitos e ausência de mensagens de erro no navegador. Não houve execução de cenários de contratos durante a criação da homepage.

Publicação concluída em 2026-10-19 UTC (2026-10-18 local): release `04a1473`, HTTPS 200 em IPv4/IPv6, HTTP 308 para HTTPS, `www` 301 para o domínio principal, assets 200 e caminho `private-output/` 404. Caddy 2.11.7 ativo e habilitado, com certificados Let's Encrypt. A página em produção foi aberta no navegador, com fonte e imagem carregadas e sem erros no console.
