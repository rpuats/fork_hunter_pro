# OpenClaw Workflow for fork_hunter_pro

Практичный workflow для параллельной разработки без хаоса в mixed repo.

## 1. Что здесь главное

Основной путь разработки — **Rust workspace** в `crates/`.

Текущий workspace включает:

- `crates/shared` — общие модели, конфиг, ошибки
- `crates/engine` — математика, нормализация, детекторы
- `crates/persistence` — история, кэш, storage-слой
- `crates/scanner` — orchestration рантайма сканера
- `crates/api` — HTTP/WebSocket слой
- `crates/bot` — Telegram
- `crates/parsers` — парсеры БК
- `crates/fork_hunter_bin` — основной entrypoint
- `crates/corridor_scanner` — отдельный corridor flow
- `crates/express_forks` — express-related logic
- `crates/bankroll_manager` — bankroll domain
- `crates/bonus_hunter` — bonus hunting domain
- `crates/auto_betting` — auto-betting domain

Legacy Python в корне сохраняется как:

- референс поведения старой системы
- набор старых тестов/утилит
- временная площадка для сравнения парсеров и миграции

Не смешивай несколько агентов в одном рабочем дереве.

---

## 2. Рекомендуемое разбиение на worktrees

Создай отдельные worktrees под роли.

> Важно: `git worktree` нормально работает только после первого коммита в репозитории.

```powershell
.\worktrees.ps1
New-ForkHunterSwarm -Bootstrap
```

Эквивалент вручную:

```powershell
New-AgentWorktree -Name rust-core -Bootstrap
New-AgentWorktree -Name parsers -Bootstrap
New-AgentWorktree -Name api-bot -Bootstrap
New-AgentWorktree -Name integration -Bootstrap
New-AgentWorktree -Name legacy-python -Bootstrap
```

### Роли

- `rust-core`
  - `crates/shared`
  - `crates/engine`
  - `crates/scanner`
  - `crates/persistence`
- `parsers`
  - `crates/parsers`
  - выборочные fixtures/tests для парсеров
- `api-bot`
  - `crates/api`
  - `crates/bot`
  - `crates/fork_hunter_bin`
- `integration`
  - cross-crate wiring
  - smoke tests
  - docs / final polish
- `legacy-python`
  - root Python scripts
  - старые `tests/`
  - сравнение поведения до/после миграции

---

## 3. Orchestrator → workers

### Orchestrator делает

- формулирует 2-4 независимые задачи
- создаёт worktree на каждую задачу
- пишет brief через `Initialize-AgentWorkspace`
- запускает worker-agent только внутри его worktree
- сводит результаты обратно в основной root/worktree

### Worker делает

- работает только в своей области
- не трогает чужие crates
- не делает cleanup legacy Python без прямой задачи
- валидирует только свой slice
- оставляет краткий итог в `agent-output.md`

---

## 4. Быстрый старт worker-worktree

```powershell
.\worktrees.ps1
Initialize-AgentWorkspace -Name parsers -Task "Улучшить parser factory / bookmaker coverage и прогнать parser tests"
```

Это создаст внутри worktree:

- `AGENT_TASK.md` — локальный brief
- `agent-output.md` — шаблон результата
- `.env` из `.env.example`, если отсутствует

---

## 5. Правила границ

### Можно менять свободно

- Rust crates
- docs по Rust migration
- bootstrap/worktree scripts
- workflow docs

### Осторожно

- корневые Python-скрипты
- historical fixtures/json/html/log files
- старые benchmark/debug artifacts
- root package (`.`), если задача реально про legacy/runtime bridge

### Не делать без явной цели

- массовое удаление legacy Python
- переписывание старых тестов только ради чистоты
- перенос всех артефактов по всему repo одним махом
- запуск нескольких агентов в одном checkout

---

## 6. Локальная валидация по ролям

### rust-core

```powershell
cargo check
cargo test -p shared -p engine -p scanner -p persistence
```

### parsers

```powershell
cargo check -p parsers
cargo test -p parsers
```

### api-bot

```powershell
cargo check -p api -p bot -p fork_hunter_bin
cargo run -p fork_hunter_bin
```

### integration

```powershell
cargo check --workspace
cargo test --workspace
```

### legacy-python

```powershell
py -m pip install -r requirements.txt
pytest tests
```

---

## 7. OpenClaw / Codex / Claude pattern

### Codex

Используй в worktree, а не в root с несколькими параллельными задачами.

Пример:

```powershell
codex exec --full-auto "Work only on crates/parsers. Improve parser health checks and update docs."
```

### Claude Code

```powershell
claude --permission-mode bypassPermissions --print "Work only on crates/api and crates/bot."
```

Смысл один: **одна роль = одно дерево = один агент**.

---

## 8. Что сейчас считается хорошим next step

1. Держать Rust как mainline
2. Любые крупные задачи дробить по crates/domain areas
3. Использовать integration-worktree для сборки итогов
4. Обновлять docs по фактической, а не исторической структуре repo
5. Оставлять legacy Python как reference, пока migration boundaries ещё живы

---

## 9. Suggested task queue

- `rust-core`: привести architecture/docs к реальному workspace
- `parsers`: стабилизировать parser factory + coverage по букмекерам
- `api-bot`: сверить endpoints/WS/state wiring с runtime
- `integration`: smoke-run `fork_hunter_bin`, собрать known issues
- `legacy-python`: сократить шум только через documented keep/remove policy
