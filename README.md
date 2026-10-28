# jogar

Abre um dos jogos LÖVE de `~/lua` pelo terminal, sem entrar na pasta.

```bash
jogar                                              # painel: setas escolhem, Enter abre, Esc ou q sai
jogar campcrawler                                  # abre direto, com o terminal do jogo
jogar campcrawler --config=campcrawler.lang=pt_BR  # o resto vai ao lançador, depois da pasta
```

Não abre o LÖVE por conta própria: chama o lançador de cada projeto, de dentro da pasta dele, e
é esse lançador que confere o runtime contra o `runtime.lock`. Precisa do `$LOVE_RUNTIME`, como
os jogos.

## O painel

Sem argumento, o `jogar` ocupa o terminal: a lista dos jogos à esquerda e, à direita, a pasta, o
lançador, a branch, as mudanças sem commit e o último commit do escolhido.

```text
╭ jogar ────────────────────────────────────────────────────────── tela X :0 ✓ ╮
│╭ Jogos ──────────╮╭ Boomlitude ─────────────────────────────────────────────╮│
││  BomberBoom     ││ pasta     ~/lua/boomlitude                              ││
││▶ Boomlitude ●   ││ lançador  luajit ferramentas/love/lanca.lua .           ││
││  CampCrawler    ││ branch    r0-pendencias ● em obra                       ││
││                 ││ mudanças  5 mudanças sem commit                         ││
││                 ││ último    48f0ad1 · há 12m                              ││
││                 ││           O mostrado acompanha as passagens (A8)        ││
│╰─────────────────╯╰─────────────────────────────────────────────────────────╯│
│ ↑↓ escolhe   Enter abre   Esc sai                                            │
╰──────────────────────────────────────────────────────────────────────────────╯
```

O lançador abre o jogo como ele está na pasta. Quando alguém está trabalhando nela, é essa obra
que abre, não o ramo principal (o do `origin/HEAD`, ou `main`). Por isso o `●` marca o jogo cuja
pasta está fora do principal ou tem mudanças sem commit. A marca só informa: o Enter abre igual.

Enquanto o jogo roda na janela dele, o painel mostra o que ele está fazendo: uma linha por tipo
de fato que o jogo escreve (`CAMPCRAWLER MAPA numero=1 linhas=9`), atualizada no lugar, com
quantas vezes veio, há quanto tempo e o último valor. A linha que acabou de mudar sai com `▸`; o
`CONFIG` mostra só o que saiu do padrão; a primeira linha do stderr fica embaixo, em vermelho. O
`x` encerra o jogo; quando a janela fecha, a lista volta com um recado do que aconteceu.

```text
╭ CampCrawler · jogando há 02:14 ───────────────────────── 412 fatos ▁▂▅▇▃▂▄ ╮
│ TIPO         VEZES  HÁ   O ÚLTIMO                                          │
│ PARTIDA           1 2m   semente 1 · idioma pt_BR · ritmo normal           │
│ CONFIG           37 2m   todas no padrão                                   │
│ MAPA              1 2m   numero 1 · linhas 9 · esperado 9                  │
│▸VEZ              24 0s   soldado 2 · texto 2º · marcado sim                │
│ x encerra o jogo · ↑↓ rola · o painel volta quando a janela fechar         │
╰────────────────────────────────────────────────────────────────────────────╯
```

## Sem tela X

Os três lançadores forçam o X11 (`SDL_VIDEO_DRIVER=x11`). Antes de abrir, o `jogar` confere se
há um X atendendo no `DISPLAY` deste terminal. Se não houver, o painel avisa no título e embaixo,
e o Enter não abre nada; `jogar <jogo>` sai com 2 numa linha, em vez do stack trace do LÖVE
("x11 not available"). Um terminal novo, ou `export DISPLAY=:0`, resolve.

Sob o Hyprland, o XWayland atende pelo `/tmp/.X11-unix/X0_`, e um `Xvfb -displayfd` de
ferramenta de teste pode apagar o `X0` ao tomar o `:0`. Quando o `X0` some mas o `X0_` atende, o
aviso diz o conserto:

```bash
ln /tmp/.X11-unix/X0_ /tmp/.X11-unix/X0
```

## Os jogos

| Jogo        | Lançador do projeto                      |
|-------------|------------------------------------------|
| BomberBoom  | `tools/love .`                           |
| Boomlitude  | `luajit ferramentas/love/lanca.lua .`    |
| CampCrawler | `tools/love .`                           |

Se um projeto mudar o lançador, ou para pôr outro jogo, mude a tabela `JOGOS` em
[`src/jogos.rs`](src/jogos.rs) e reinstale.

## Instalar

```bash
cargo install --path ~/love-lua-cli
```
