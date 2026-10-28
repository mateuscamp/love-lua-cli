# jogar

Abre um dos jogos LÖVE de `~/lua` pelo terminal, sem entrar na pasta.

```bash
jogar                                              # menu: setas escolhem, Enter abre, Esc ou q sai
jogar campcrawler                                  # abre direto
jogar campcrawler --config=campcrawler.lang=pt_BR  # o resto vai ao lançador, depois da pasta
```

Não abre o LÖVE por conta própria: chama o lançador de cada projeto, de dentro da pasta dele, e
é esse lançador que confere o runtime contra o `runtime.lock`. Precisa do `$LOVE_RUNTIME`, como
os jogos. Quando o jogo fecha, o menu volta.

Os três lançadores forçam o X11 (`SDL_VIDEO_DRIVER=x11`). Antes de abrir, o `jogar` confere se
há um X atendendo no `DISPLAY` deste terminal; se não houver, o menu avisa no lugar da pergunta e
o Enter não abre nada, e `jogar <jogo>` sai com 2 numa linha, em vez do stack trace do LÖVE
("x11 not available"). Um terminal novo, ou `export DISPLAY=:0`, resolve.

| Jogo        | Lançador do projeto                      |
|-------------|------------------------------------------|
| BomberBoom  | `tools/love .`                           |
| Boomlitude  | `luajit ferramentas/love/lanca.lua .`    |
| CampCrawler | `tools/love .`                           |

Se um projeto mudar o lançador, ou para pôr outro jogo, mude a tabela `JOGOS` em
[`src/main.rs`](src/main.rs) e reinstale.

## Instalar

```bash
cargo install --path .
```
