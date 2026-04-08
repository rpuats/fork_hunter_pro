# OpenClaw Workflow for fork_hunter_pro

Практичный workflow для параллельной разработки в этом репозитории.

## 1. Что здесь главное

Основной путь разработки — **Rust workspace**:

- `crates/shared` — общие модели, конфиг, ошибки
- `crates/engine` — математика, нормализация, детекторы
- `crates/parsers` — парсеры БК
- `crates/scanner` — orchestration рантайма сканера
- `crates/persistence` — история и кэш
- `crates/api` — HTTP/WebSocket слой
- `crates/bot` — Telegram
- `crates/fork_hunter_bin` — точка входа

Legacy Python в корне сохраняется как:

- референс поведения
- набор старых тестов/утилит
- временная площадка для сравнения парсеров

Не смешивай несколько агентов в одном рабочем дереве.

---

## 2. Рекомендуемое разбиение на worktrees

Создай отдельные worktrees под роли.

> Важно: `git worktree` начнёт нормально работать только после первого коммита в репозитории.


```powershell
.\worktrees.ps1
New-AgentWorktree -Name rust-core
New-AgentWorktree -Name parsers
New-AgentWorktree -Name api-bot
New-AgentWorktree -Name integration
```

### Роли

- `rust-core`
  - `crates/shared`
  - `crates/engine`
  - `crates/scanner`
  - `crates/persistence`
- `parsers`
  - `crates/parsers`
  - выборочные fixture/tests для парсеров
- `api-bot`
  - `crates/api`
  - `crates/bot`
  - `crates/fork_hunter_bin`
- `integration`
  - cross-crate wiring
  - smoke tests
  - docs / final polish

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
- валидирует только свой slice
- оставляет краткий итог в `agent-output.md`

---

## 4. Быстрый старт worker-worktree

```powershell
.\worktrees.ps1
Initialize-AgentWorkspace -Name parsers -Task "Улучшить factor catalog / parser factory и прогнать parser tests"
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

### Не делать без явной цели

- массовое удаление legacy Python
- переписывание старых тестов только ради чистоты
- перенос всех артефактов по всему repo одним махом

---

## 6. Локальная валидация по ролям

### rust-core

```powershell
cargo check
cargo test -p engine -p shared -p scanner -p persistence
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
2. Любые крупные задачи дробить по crates
3. Использовать integration-worktree для сборки итогов
4. Обновлять docs по фактической, а не исторической структуре repo

---

## 9. Suggested task queue

- `rust-core`: вычистить architecture docs под реальный workspace
- `parsers`: стабилизировать parser factory + coverage по букмекерам
- `api-bot`: сверить endpoints/WS/state wiring с runtime
- `integration`: smoke-run `fork_hunter_bin`, собрать known issues
