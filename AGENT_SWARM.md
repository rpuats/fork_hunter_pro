# Agent Swarm

Боевой контур для параллельной разработки `fork_hunter_pro`.

## Текущее состояние

- Базовая ветка swarm: `codex-swarm-base-20260416-0343`
- Checkpoint commit: `c1e389d`
- Основной control plane живёт в main checkout, рабочие слоты идут в clean `.worktrees/swarm-*`
- Практический лимит этой сессии: одновременно держать не больше `6` живых агентов

## Главный принцип

Один агент = одно worktree = одна зона ответственности.

Не смешивать:

- parser work по разным БК
- core safety/runtime work
- operator/API/UI work
- execution/freebet/bankroll work
- legacy Python reference support

## Активная схема

### Активная волна

- `swarm-winline`
- `swarm-melbet`
- `swarm-betboom`
- `swarm-ligastavok`
- `coordinator` (read-only dispatcher)
- `service` (rotating shared slot)

### Следующая очередь

- `swarm-core-safety`
- `swarm-api-operator`
- `swarm-ui-operator`
- `swarm-execution-money`
- `swarm-agent-improvement`
- `swarm-legacy-python`

## Worktree map

| Worktree | Ownership | Goal |
|---|---|---|
| `swarm-winline` | Winline parser, diagnostics, tests | Сузить runtime blocker, убрать дорогой fanout, вывести честный bounded path |
| `swarm-melbet` | Melbet parser, diagnostics, tests | Продвинуть route/bootstrap path к production-ready feed |
| `swarm-betboom` | Betboom parser, diagnostics, tests | Довести diagnostic-ready parser до runtime feed |
| `swarm-ligastavok` | LigaStavok parser, diagnostics, tests | Усилить honest blocker/readiness вокруг anti-bot session bootstrap |
| `swarm-core-safety` | `crates/shared`, `crates/engine`, `crates/scanner`, `crates/persistence` | Verifier, bulkhead, validator, health, runtime truthfulness |
| `swarm-api-operator` | `crates/api`, `crates/bot`, `crates/fork_hunter_bin` | Readiness, triage, status surfaces, operator contracts |
| `swarm-ui-operator` | `desktop-ui/src` | Operator cockpit, parser health, execution/funding surfaces |
| `swarm-execution-money` | `crates/auto_betting`, `crates/bonus_hunter`, `crates/bankroll_manager` | Safe semi-auto groundwork, bankroll/funding readiness |
| `swarm-agent-improvement` | bootstrap/workflow/docs/scripts | Улучшение swarm loop, памяти, handoff и bounded execution |
| `swarm-legacy-python` | root Python scripts/tests/tooling | Только comparison, migration support, behavior reference |

## Done criteria

### Для parser worker

- Найден реальный runtime blocker, а не общий vague fail
- Добавлены или уточнены diagnostics/guardrails
- Изменение делает поведение либо более рабочим, либо более честно bounded
- Оставлены локальные шаги проверки
- Ясно указано, что остаётся внешним blocker

### Для service worker

- Улучшение имеет узкий ownership
- Изменения не лезут в чужие worktrees/домены
- Есть локальная проверка slice
- Есть короткий handoff для следующей волны

## Минимальный цикл

1. Взять своё `swarm-*` worktree.
2. Прочитать `docs/memory/README.md`, `docs/onboarding/MULTI_SESSION_SWARM.md`, `docs/onboarding/SWARM_STATUS.md`.
3. Сделать только scoped changes.
4. Прогнать только свой validation slice.
5. Описать итог и остаточные blockers.
6. Освободить слот для следующей роли.

## Safe validation

По умолчанию:

```powershell
git status --short
cargo check -p shared
cargo check -p engine
cargo check -p persistence
cargo check -p scanner
cargo check -p parsers
py -m pytest --collect-only -q
```

После стабилизации отдельного slice:

```powershell
cargo test -p shared --lib --quiet
cargo test -p engine --lib --quiet
cargo test -p persistence --lib --quiet
cargo test -p scanner --lib --quiet
cargo test -p parsers --lib --quiet
```

## Unsafe by default

Не запускать без явной причины:

- `cargo test --workspace`
- root `test_*.py` вне целевого slice
- browser discovery scripts
- heavy artifact generators
- live runtime against real credentials
- broad cleanup старых Python/diagnostic файлов

## Локальная память

Истину держать в компактных repo-файлах, а не в длинной истории чата:

- `DEV_SETUP.md`
- `docs/onboarding/AUTONOMOUS_SWARM.md`
- `desktop-ui/README.md`
- `config.yaml.example`
- `AGENT_SWARM.md`
- `docs/memory/*`
- `COMPRESSION.md`
