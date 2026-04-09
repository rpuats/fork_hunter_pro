# Agent Swarm

Боевой контур для параллельной разработки `fork_hunter_pro`.

## Что уже настроено

- `bootstrap.ps1` — быстрый/full bootstrap dual-stack окружения
- `worktrees.ps1` — создание и инициализация изолированных worktrees
- `New-ForkHunterSwarm -Bootstrap` — готовый расклад ролей под проект
- `.worktrees/` уже создан и заполнен worktree-ветками:
  - `rust-core`
  - `parsers`
  - `api-bot`
  - `integration`
  - `legacy-python`

## Быстрый старт

```powershell
. .\worktrees.ps1
Show-ForkHunterSwarm
```

Если клон новый и worktrees ещё не созданы:

```powershell
. .\worktrees.ps1
New-ForkHunterSwarm -Bootstrap
```

## Роли

### rust-core
- scope: `crates/shared`, `crates/engine`, `crates/scanner`, `crates/persistence`
- задачи: runtime core, performance, correctness
- не лезет в parsers/api/bot без отдельной задачи

### parsers
- scope: `crates/parsers`
- задачи: bookmaker coverage, parser_factory, normalization, reliability

### api-bot
- scope: `crates/api`, `crates/bot`, `crates/fork_hunter_bin`
- задачи: HTTP/WS, bot wiring, runtime entrypoint glue

### integration
- scope: workspace-wide validation, smoke tests, docs sync
- задача: не фичи, а сборка итогов и final verification

### legacy-python
- scope: root Python scripts/tests
- задача: только migration/reference/support
- не использовать как mainline

## Правило №1

Один агент = одно worktree = один scope.

## Рекомендуемый запуск

1. `rust-core` и `parsers` стартуют первыми
2. `api-bot` идёт параллельно, если не зависит от parser changes
3. `integration` включается после первых merge-ready результатов
4. `legacy-python` запускать только при сравнении поведения или миграционных вопросах

## Skills / plugins

Отобраны и установлены только практичные штуки:

- `agent-team-orchestration` — роль/процесс/handoff слой
- `git-worktree-manager` — полезный git-worktree паттерн
- `codex-orchestrator` — установлен как слой управления Codex runs

Сознательно **не установлен автоматически**:
- `codex-sub-agents` — ClawHub пометил как suspicious; без ручного ревью форсить не стоит

## Что читать агентам

- `AGENT_TASK.md` внутри своего worktree
- `agent-output.md` внутри своего worktree
- `DEV_SETUP.md`
- `OPENCLAW_WORKFLOW.md`

## Минимальный цикл работы

1. взять worktree
2. прочитать `AGENT_TASK.md`
3. сделать scoped changes
4. прогнать только свой validation slice
5. записать итог в `agent-output.md`
6. отдать в integration
