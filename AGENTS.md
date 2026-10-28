# jogar

CLI do autor: abre os jogos LÖVE de `~/lua` pelo lançador de cada projeto. Uso: `README.md`.
Regra nova entra aqui com `(autor, dd/mm)`; o porquê, no commit. Sem `CLAUDE.md` aqui (o Claude Code
ignoraria este arquivo).

## Comandos

- Portão: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- Requer Rust 1.88+ (`rust-version`), git, `script` (util-linux). Jogo: `xvfb-run`, `luajit`,
  `$LOVE_RUNTIME`.
- Instalar: `cargo install --path ~/love-lua-cli`, só do `main` enviado.
- Painel à vista: `tmux -L <sock> new-session -d -x 100 -y 30 "env ... jogar"`, `send-keys`,
  `capture-pane -p`, dentro do `xvfb-run -a`.
- Caixa (auditoria): `bwrap --ro-bind / /`, HOME em tmpfs, `--unshare-all`. Com jogo: `~/lua` e
  `~/.local/opt/love` só leitura, `--tmpfs /var/lib/xkb`, `/tmp/.X11-unix` 1777,
  `__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json`,
  `__GLX_VENDOR_LIBRARY_NAME=mesa`.

## Limites

- Não abrir o LÖVE nem decidir regra de jogo: chamar o lançador que o `AGENTS.md` do jogo manda
  (tabela `JOGOS`). Não é o hub descartado em 23/10. Mora aqui, não no `~/love-lua` (ADR 0003).
- `~/lua/<jogo>`: só leitura, `git --no-optional-locks`. Nunca checkout, stash, pull ou escrita.
- Linha do Boomlitude em `JOGOS`: não mudar até a orquestração dele mandar a nova (28/10).
- Jogo só sob `xvfb-run -a` (display ≥ `:99`), som nulo, XDG em pasta temporária. Nunca
  `Xvfb -displayfd`: toma o `:0` e apaga o `X0` do autor (28/10). Tela do autor só com pergunta.
- Sobra de processo: conferir por grupo (`pgrep -g`), nunca `pgrep -x love` (há LÖVE de outra
  sessão).
- Produto (autor, 28/10): painel em tela cheia; estado ao vivo durante o jogo; obra se mostra, não
  se impede; aviso do X0 com o `ln`.

## Mapa

- `main.rs`: argumentos; painel ou `jogar <jogo>` direto.
- `jogos.rs`: `JOGOS`; `abre` (direto); `sobe` (cano, stdin nulo, grupo próprio).
- `tela.rs`: X antes de abrir. `obra.rs`: git das pastas. `fatos.rs`: linhas do jogo, `Placar`.
- `painel/`: `mod.rs` laço e lista (único com terminal); `processo.rs` jogo rodando (único com
  processo); `sinais.rs`; `lista.rs`, `partida.rs` só desenham; `teclas.rs`, `formato.rs` puros.
- `apoio_de_teste.rs`: ajudantes; `tests/` o inclui por `#[path]`.

## Código (Clean Code para agente, Akita)

- Arquivo: alvo 300 linhas, teto 500 com testes. Função: alvo 20, teto 40. Passou: dividir.
- Nome em português, único no `rg -nw`. Proibido: `dados`, `info`, `util`, `handler`.
- Tipos explícitos. Erro ao autor: `Result<_, String>`, uma linha, valor e esperado.
- `unwrap`/`expect` fora de teste: só invariante, com o porquê.
- Retorno cedo; no máximo 2 níveis de indentação.
- Mundo por parâmetro (raiz, display, sockets); só invólucro fino lê `env`.
- `ratatui` só em `painel/`; desenho recebe `Vista…` pronta.
- `///` em toda função: intenção e limite. Comentário: porquê e proveniência. Manter comentário
  existente; tirar só o falso.

## Defesa (manter)

- Sem X: avisar e não abrir. `X<n>` sumido e `X<n>_` vivo: o aviso traz o `ln`.
- Pasta ou lançador ausente; jogo que morre: uma linha com código e 1ª linha do stderr.
- Git falha: sem marca. Sem tty: sair 2.
- `x`: SIGTERM no grupo. Ctrl+C: SIGTERM, 2 s, SIGKILL, sair 130. Pânico: o `Drop` da `Partida`.
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
- Só descartável: `pasta_de_teste`, `git_teste`, `jogo_falso`, `UnixListener` como X. Nunca
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
