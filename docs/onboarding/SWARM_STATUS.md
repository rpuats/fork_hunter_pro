# Swarm Status

Текущий автономный цикл для `fork_hunter_pro`.

## Base snapshot

- Branch: `codex-swarm-base-20260416-0343`
- Commit: `c1e389d`
- Main checkout можно использовать как orchestration/control plane
- Реальная разработка идёт в clean `.worktrees/swarm-*`
- Lane/task control идёт через `scripts/swarm_control.py` и `config/swarm/tasks.json`

## Live-agent limit

Практический лимит активных агентов в этой сессии: `6`.

Поэтому swarm работает волнами:

1. четыре parser worker
2. один dispatcher/read-only coordinator
3. один rotating service slot

## Wave 1

| Role | Worktree | Goal |
|---|---|---|
| Winline worker | `.worktrees/swarm-winline` | Убрать дорогой fanout, сузить runtime blocker |
| Melbet worker | `.worktrees/swarm-melbet` | Продвинуть route/bootstrap path к реальному feed |
| Betboom worker | `.worktrees/swarm-betboom` | Довести diagnostic-ready parser до runtime-feed path |
| LigaStavok worker | `.worktrees/swarm-ligastavok` | Усилить honest blocker/readiness вокруг anti-bot session bootstrap |
| Coordinator | read-only | Выдаёт compact swarm board и next tasks |
| Service slot | rotating | `core-safety`, `api/operator`, `execution-money` и `agent-improvement` закрыты; слот свободен для следующего bounded improvement |

## Wave 2 queue

- `.worktrees/swarm-core-safety`
- `.worktrees/swarm-api-operator`
- `.worktrees/swarm-ui-operator`
- `.worktrees/swarm-execution-money`
- `.worktrees/swarm-agent-improvement`
- `.worktrees/swarm-legacy-python`

## Rotation rule

- Если parser worker закончил локальный bounded patch, его слот сразу отдаётся следующему приоритетному worker.
- Если worker упёрся во внешний blocker, он обязан оставить honest handoff: `what works`, `what failed`, `what remains external`.
- Service slot всегда получает задачу, которая повышает общий throughput всей системы, а не локально украшает код.
- Handoff и живая repo-память идут через `docs/memory/*` и `COMPRESSION.md`, а не через длинные чат-сводки.

## Current priorities

- Current active wave stays at four parser lanes plus coordinator plus one rotating service lane.
- Completed in this wave: `melbet-bootstrap-truth`, `betboom-feed-path`, `ligastavok-session-readiness`, `service-core-truthfulness`, `service-operator-readiness`, `service-execution-safety`, `service-agent-throughput`, `coordinator-wave-board`, `winline-fanout-bounds`.

1. `winline`
2. `melbet`
3. `betboom`
4. `ligastavok`
5. `ui/operator`
6. `api/operator-followup`
7. `legacy-python`
8. `next bounded service patch`
