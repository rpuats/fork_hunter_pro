# OpenClaw Stack for fork_hunter_pro

Практичная настройка OpenClaw под mixed repo: Rust mainline + legacy Python рядом.

## Что уже есть в репо

Проектный слой для агентной работы уже оформлен через:

- `bootstrap.ps1`
- `worktrees.ps1`
- `DEV_SETUP.md`
- `OPENCLAW_WORKFLOW.md`

## Что важно понимать

- главный вектор разработки — Rust workspace в `crates/`
- legacy Python в корне пока не удалён и нужен как reference/tooling layer
- root checkout шумный, поэтому для параллельной работы нужны именно worktrees

## Минимальный боевой контур

1. `./bootstrap.ps1`
2. `./worktrees.ps1`
3. `New-ForkHunterSwarm -Bootstrap`
4. orchestrator раздаёт scoped tasks
5. workers оставляют итог в `agent-output.md`
6. integration собирает и валидирует итог

Уже созданные worktrees:
- `.worktrees/rust-core`
- `.worktrees/parsers`
- `.worktrees/api-bot`
- `.worktrees/integration`
- `.worktrees/legacy-python`

## Рекомендуемая архитектура агентов

### orchestrator
- держит общий план
- создаёт worktrees
- пишет задачи в `AGENT_TASK.md`
- собирает результат из `agent-output.md`
- следит, чтобы задачи не пересекались по scope

### workers
- `rust-core`
- `parsers`
- `api-bot`
- `integration`
- `legacy-python` при явной необходимости

Каждый worker работает только в своём worktree.

## Когда использовать что

### `clawflow`
Когда работа должна идти как одна логическая задача, но с несколькими отдельными шагами или исполнителями.

### `coding-agent`
Когда задача реально кодовая и достаточно объёмная, чтобы отдавать её отдельному агенту.

### Codex / Claude CLI
Когда нужен отдельный автономный run на узкий scope внутри конкретного worktree.

## Практические правила

- не запускать несколько агентов в одном рабочем дереве
- не смешивать cleanup legacy Python с Rust mainline задачами
- не валидировать весь repo, если менялся один crate или один doc/script slice
- доверять `Cargo.toml` и `crates/` больше, чем старым Python-first описаниям

## Что bootstrap реально делает

- копирует `.env.example` в `.env`, если нужно
- ставит Python requirements для legacy tooling/reference scripts
- делает `cargo check`
- в full режиме гоняет focused Rust tests по основным crate'ам
- при наличии `npm` ставит Codex CLI и Claude Code CLI
- по флагу `-InstallOptionalTools` ставит `cargo-nextest` и `cargo-watch`

## Установленные skills / plugins

Поставлены только те, которые реально усиливают поток работы:

- `agent-team-orchestration`
- `git-worktree-manager`
- `codex-orchestrator`

Сознательно не был форсирован:
- `codex-sub-agents` — помечен ClawHub как suspicious, без ручного ревью ставить не стоит

## Suggested next OpenClaw tasks

1. Держать docs синхронными с реальной формой workspace
2. Разбивать крупные задачи по crates/domain areas
3. Оставить отдельный `legacy-python` поток только для миграции и сравнения поведения
4. При росте количества параллельных задач использовать `New-ForkHunterSwarm` и `AGENT_SWARM.md`, а не усложнять root checkout
