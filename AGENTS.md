# jogar

CLI do autor: abre os jogos LÖVE de `~/lua` pelo lançador comum, `tools/love`. Uso: `README.md`.
Regra nova entra aqui com `(autor, dd/mm)`; o porquê, no commit. Sem `CLAUDE.md` aqui (o Claude Code
ignoraria este arquivo).

## Comandos

- Portão: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- Requer Rust 1.88+ (`rust-version`), git, `script` (util-linux). Jogo: `xvfb-run`, `luajit`,
  `$LOVE_RUNTIME`.
- Instalar: `cargo install --path ~/love-lua-cli`, só do `main` enviado.
- Painel à vista: `tmux -L <sock> new-session -d -x 100 -y 30 "env ... jogar"`, `send-keys`,
  `capture-pane -p`, dentro do `xvfb-run -a`.
- Cor à vista: `kitty --config NONE ... jogar` sob `xvfb-run -a` na caixa, `LIBGL_ALWAYS_SOFTWARE=1`,
  `xdotool key`, `import -window root`.
- Caixa (auditoria): `bwrap --ro-bind / /`, HOME em tmpfs, `--unshare-all`. Com jogo: `~/lua` e
  `~/.local/opt/love` só leitura, `--tmpfs /var/lib/xkb`, `/tmp/.X11-unix` 1777,
  `__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json`,
  `__GLX_VENDOR_LIBRARY_NAME=mesa`.

## Limites

- Não abrir o LÖVE nem decidir regra de jogo: chamar `tools/love .` da pasta do jogo. Não é o hub
  descartado em 23/10. Mora aqui, não no `~/love-lua` (ADR 0003).
- Jogo é pasta da raiz (`$JOGAR_RAIZ` ou `~/lua`) com `tools/love` executável; nome é a pasta. As
  regras de um jogo (lançador igual em todos, X11, fatos) moram no README, "Adding a game"; mudou
  uma, muda lá (autor, 31/10).
- `~/lua/<jogo>`: só leitura, `git --no-optional-locks`. Nunca checkout, stash, pull ou escrita.
- Jogo só sob `xvfb-run -a` (display ≥ `:99`), som nulo, XDG em pasta temporária. Nunca
  `Xvfb -displayfd`: toma o `:0` e apaga o `X0` do autor (28/10). Tela do autor só com pergunta.
- Sobra de processo: conferir por grupo (`pgrep -g`), nunca `pgrep -x love` (há LÖVE de outra
  sessão).
- Produto (autor, 28/10): painel em tela cheia; estado ao vivo durante o jogo; obra se mostra, não
  se impede; aviso do X0 com o `ln`. Cores anil e magenta, em 24 bits (autor, 30/10).
- `NAO_JOGAVEL` na pasta do jogo (autor, 01/11): o jogo declara que ainda não se joga (motivo na 1ª
  linha); o painel marca `✕` e mostra, mas abre. `● em obra` é só git (branch fora do main).
- Portfólio (autor, 31/10): repositório público, GPL-3.0; README em inglês com resumo em português.
  Código e comentários em inglês; o texto da tela segue em português (autor, 31/10). `≠` marca o
  lançador que destoa da maioria; só avisa.

## Mapa

- `main.rs`: argumentos e raiz; painel ou `jogar <jogo>` direto.
- `games.rs`: `discover`, `find_game`; `run_attached` (direto); `spawn_piped` (cano, stdin nulo,
  grupo próprio).
- `shared_launcher.rs`: a cópia do lançador em cada jogo e quais destoam.
- `x_display.rs`: X antes de abrir. `git_state.rs`: git das pastas. `facts.rs`: linhas do jogo,
  `Scoreboard`.
- `panel/`: `mod.rs` laço e lista (único com terminal); `game_process.rs` jogo rodando (único com
  processo); `signals.rs`; `list_screen.rs`, `live_screen.rs` só desenham; `theme.rs` cores e
  degradê; `keys.rs`, `formatting.rs` puros; `draw_test_support.rs` só nos testes.
- `test_support.rs`: ajudantes; `tests/` o inclui por `#[path]`.

## Código (Clean Code para agente, Akita)

- Arquivo: alvo 300 linhas, teto 500 com testes. Função: alvo 20, teto 40. Passou: dividir.
- Nome em inglês, único no `rg -nw` (autor, 31/10). Proibido: `data`, `info`, `util`, `handler`.
- Tipos explícitos. Erro ao autor: `Result<_, String>`, uma linha, valor e esperado.
- `unwrap`/`expect` fora de teste: só invariante, com o porquê.
- Retorno cedo; no máximo 2 níveis de indentação.
- Mundo por parâmetro (raiz, display, sockets); só invólucro fino lê `env`.
- `ratatui` só em `panel/`; desenho recebe `…View` pronta.
- Cor só pelas constantes e pintores de `theme.rs`; nada de `.red()`, `.green()` solto.
- `///` em toda função: intenção e limite. Comentário: porquê e proveniência. Manter comentário
  existente; tirar só o falso.

## Defesa (manter)

- Sem X: avisar e não abrir. `X<n>` sumido e `X<n>_` vivo: o aviso traz o `ln`.
- Pasta que some depois da descoberta; jogo que morre: uma linha com código e 1ª linha do stderr.
- Raiz sem jogo: o painel diz onde procurou e o que é um jogo; as teclas não quebram.
- Git falha: sem marca. Sem tty: sair 2.
- `x`: SIGTERM no grupo. Ctrl+C: SIGTERM, 2 s, SIGKILL, sair 130. Pânico: o `Drop` do `RunningGame`.
- SIGHUP/SIGTERM no jogar: o tratador manda SIGTERM ao grupo do jogo. SIGHUP sai ali (`_exit`): no
  tty fechado o crossterm gira em falso e o laço não volta. SIGTERM sai pelo laço.
- Ainda deixa órfão: SIGKILL, ou SIGINT/SIGQUIT de fora, no jogar.
- Tratador de sinal: só atômico, `killpg`, `_exit`.

## Testes (skill `code-audit`, 4.3)

- Comportamento novo ou bug: teste na suíte, no mesmo commit, visto falhar pela asserção antes.
  Commit diz comando, vermelho, verde.
- Teste alterado: justificar no commit; expectativa antiga ainda válida ganha outro teste.
- Exceções, só declaradas no commit: refatoração (a suíte é a rede); aparência da tela (captura,
  desenho testado no `TestBackend`).
- Só descartável: `test_folder`, `test_git`, `fake_game`, `UnixListener` como X. Nunca
  `~/lua`, `~/.cargo`, tela do autor.

## Limpeza (Akita, "A limpeza da casa na era do AI coding")

- Ao terminar, apagar o temporário que a tarefa criou, e só ele: cópias de auditoria (com o
  `target/`), saídas de Xvfb e tmux. Não criou ou não sabe atribuir: listar ao autor.
- `target/` cresce sem limite (cada cópia, ~300 MB): `cargo clean` na worktree depois do commit
  da rodada e antes de arquivá-la.
- Limpeza ampla (`cargo sweep`, `cargo clean-all`, caches, Docker): só com o sim do autor, após
  inventário só leitura, por lista exata.

## Com o autor

- Toda rodada de código termina com a skill `code-audit` no diff; relatório antes de pedir commit.
  Achado só se corrige com o sim, um por vez, vermelho antes (autor, 28/10).
- Pergunta: múltipla escolha, recomendada em primeiro e marcada; repetir as opções no texto (ele
  lê pelo celular) (autor, 28/10).
- Regra de trabalho do autor vem para cá, não só para a memória (autor, 28/10).
- Handoff: só o último do ai-memory. É relato datado: conferir `git worktree list`, `git status`,
  `gh pr list` (autor, 28/10).
- Commit, push, merge: só com pedido. Worktree → `main` por fast-forward → GitHub → reinstalar.
  Nunca force push, histórico enviado reescrito ou `git stash` sem nome.
- Commit em português: título do que muda para o autor; corpo com porquê, tópicos e prova.
