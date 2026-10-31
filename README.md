# jogar

> **Em português.** `jogar` é um painel de terminal que abre os jogos LÖVE da pasta `~/lua` e
> mostra, ao vivo, o que cada jogo está fazendo. Jogo é toda pasta com um `tools/love` executável:
> para pôr um jogo, basta a pasta. Todo jogo usa o **mesmo lançador**, copiado igual de um projeto
> para outro; o painel marca com `≠` o jogo cujo lançador difere dos outros. As regras completas
> estão em [Adding a game](#adding-a-game). Licença GPL-3.0.

A terminal panel for a folder of [LÖVE](https://love2d.org) games. It lists the games, shows the
git state of each one, opens the chosen game through its own launcher and, while the game runs in
its window, shows live what the game reports about itself.

```bash
jogar                                              # panel: arrows choose, Enter opens, Esc or q quits
jogar campcrawler                                  # opens directly, with the game's own terminal
jogar campcrawler --config=campcrawler.lang=pt_BR  # the rest goes to the launcher, after the folder
```

`jogar` never starts LÖVE by itself: it runs the game's launcher, `tools/love`, from inside the
game's folder, and the launcher checks the LÖVE runtime against the game's `runtime.lock`.

## The panel

With no argument, `jogar` takes the whole terminal: the games on the left and, on the right, the
chosen game's folder, launcher, branch, uncommitted changes and last commit. The interface is in
Portuguese.

```text
╭ jogar ────────────────────────────────────────────────────────── tela X :0 ✓ ╮
│╭ Jogos ──────────╮╭ boomlitude ─────────────────────────────────────────────╮│
││  bomberboom ≠   ││ pasta     ~/lua/boomlitude                              ││
││▶ boomlitude ●   ││ lançador  tools/love .                                  ││
││  campcrawler    ││ branch    r0-pendencias ● em obra                       ││
││  gods           ││ mudanças  5 mudanças sem commit                         ││
││                 ││ último    48f0ad1 · há 12m                              ││
││                 ││           O mostrado acompanha as passagens (A8)        ││
│╰─────────────────╯╰─────────────────────────────────────────────────────────╯│
│ ↑↓ escolhe   Enter abre   Esc sai                                            │
╰──────────────────────────────────────────────────────────────────────────────╯
```

- `●` marks a game whose folder is off its main branch (`origin/HEAD`, or `main`) or has
  uncommitted changes. The launcher opens the game as it is in the folder, so that work in
  progress is what opens. The mark only informs: Enter opens it all the same.
- `≠` marks a game whose copy of the shared launcher differs from the other games' copies (see
  [Adding a game](#adding-a-game)). It also only informs.

The colors are indigo and magenta: an indigo-night background, a frame fading from indigo to
magenta, the name as a magenta tag, work in progress in orchid, what is fine in cyan and what went
wrong in coral. It needs a 24-bit color terminal (kitty, foot, ghostty, wezterm); in tmux without
`RGB` in `terminal-features`, the colors are approximated.

While the game runs in its window, the panel shows one line per kind of fact the game writes
(`CAMPCRAWLER MAPA numero=1 linhas=9`), updated in place, with how many times it came, how long
ago and its last value. The line that just changed gets a `▸`; `CONFIG` shows only what is off its
default; the first stderr line stays at the bottom, in coral. `x` closes the game; when the window
closes, the list comes back with a note on how it ended.

```text
╭ campcrawler · jogando há 02:14 ───────────────────────── 412 fatos ▁▂▅▇▃▂▄ ╮
│ TIPO         VEZES  HÁ   O ÚLTIMO                                          │
│ PARTIDA           1 2m   semente 1 · idioma pt_BR · ritmo normal           │
│ CONFIG           37 2m   todas no padrão                                   │
│ MAPA              1 2m   numero 1 · linhas 9 · esperado 9                  │
│▸VEZ              24 0s   soldado 2 · texto 2º · marcado sim                │
│ x encerra o jogo · ↑↓ rola · o painel volta quando a janela fechar         │
╰────────────────────────────────────────────────────────────────────────────╯
```

## Adding a game

The games live in `~/lua`, or in the folder `$JOGAR_RAIZ` names. **A game is any folder there with
an executable `tools/love`**: to add one, put the folder there; nothing changes in `jogar`. The
list is alphabetical and shows each game by its folder name.

Every game follows these rules:

1. **The same launcher in every game.** `tools/love` and the pieces in `tools/lancador/` are the
   same files in every project: copy them from another game, and never edit them in just one.
   What belongs to the project goes in `tools/love_projeto.lua`, which the comparison leaves out. A
   fix to the launcher is copied to all games. The panel compares the copies and marks with `≠`
   the game that differs from the majority (with no majority, every game that differs).
2. **Opened from its folder.** `jogar` runs `tools/love .` with the game's folder as the working
   directory, and appends whatever follows `jogar <game>`, such as
   `--config=<project>.<key>=<value>`.
3. **X11.** The game runs on X11 (`SDL_VIDEO_DRIVER=x11`). Before opening, `jogar` checks that an
   X server answers on `DISPLAY` (see [No X display](#no-x-display)).
4. **Facts on stdout, one per line.** Line-buffered stdout (`io.stdout:setvbuf("line")` at the top
   of `conf.lua`), so each fact arrives live. A fact is
   `<FOLDER IN UPPERCASE> <TYPE> key=value ...`, as in `GODS JANELA largura=1280 altura=720`. A
   `CONFIG` fact carries `chave=`, `valor=` and `origem=` (`padrao` for a default). Any other line
   is shown as text.
5. **SIGTERM closes it.** `x` sends SIGTERM to the game's process group, which LÖVE treats as
   closing the window, and exit code 0 reads as a normal close.

The launcher is not part of this repository: each game carries its own copy.

## No X display

The games force X11. Before opening one, `jogar` checks for an X server answering on this
terminal's `DISPLAY`. If there is none, the panel says so in the title and at the bottom, and Enter
opens nothing; `jogar <game>` exits with 2 and a one-line message instead of LÖVE's stack trace
("x11 not available"). A new terminal, or `export DISPLAY=:0`, fixes it.

Under Hyprland, XWayland answers on `/tmp/.X11-unix/X0_`, and an `Xvfb -displayfd` from a test tool
can delete `X0` when it takes `:0`. When `X0` is gone but `X0_` answers, the warning gives the fix:

```bash
ln /tmp/.X11-unix/X0_ /tmp/.X11-unix/X0
```

## Install

Linux, Rust 1.88 or later, and git. Each game brings its own runtime requirements (the LÖVE build
its launcher checks).

```bash
cargo install --path ~/love-lua-cli
```

## License

[GPL-3.0](LICENSE).
