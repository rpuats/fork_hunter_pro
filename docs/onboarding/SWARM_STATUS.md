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

## Wave 2

| Role | Worktree | Goal |
|---|---|---|
| Winline worker | `.worktrees/swarm-winline` | Проверить, может ли bounded structured path стать стабильным feed decision |
| Melbet worker | `.worktrees/swarm-melbet` | Перевести transport/runtime blocker в более узкий feed/bootstrap decision |
| Betboom worker | `.worktrees/swarm-betboom` | Продвинуть guarded runtime/Sporthub path к реальному feed либо tighter blocker |
| LigaStavok worker | `.worktrees/swarm-ligastavok` | Собрать более сильное bootstrap evidence без fake bypasses |
| Coordinator | read-only | Держит Wave 2 board, queue и compact handoff current |
| Service slot | rotating | `service-ui-operator-patch`; следующий bounded slot — `service-swarm-autonomy` |

## Wave 3

| Role | Worktree | Goal |
|---|---|---|
| Winline worker | `.worktrees/swarm-winline` | После factory/readiness test доказать stable structured feed или оставить explicit blocker |
| Melbet worker | `.worktrees/swarm-melbet` | Упаковать live bootstrap barrier в reusable evidence для browser-path/external-blocker decision |
| Betboom worker | `.worktrees/swarm-betboom` | Решить, даёт ли blocksplit/runtime-card shape реальный feed path или только diagnostic mode |
| LigaStavok worker | `.worktrees/swarm-ligastavok` | Снять один stronger browser bootstrap signal без ослабления anti-bot truth |
| Coordinator | read-only | Держит Wave 3 board compact, blocker-synced и коротким |
| Service slot | rotating | `service-parser-capability-catalog` в работе; следующий bounded slot не поднимать до handoff |

## Wave 4

| Role | Worktree | Goal |
|---|---|---|
| Winline lane | `.worktrees/swarm-winline` | Wave 3 task закрыт; follow-up не поднимать без bounded handoff |
| Melbet lane | `.worktrees/swarm-melbet` | Wave 3 task закрыт; follow-up держать в seed/backlog до promotion |
| Betboom lane | `.worktrees/swarm-betboom` | Wave 3 task закрыт; ждать узкий runtime/feed follow-up из handoff |
| LigaStavok lane | `.worktrees/swarm-ligastavok` | Wave 3 task закрыт; anti-bot truth держать explicit до promotion |
| Tennisi lane | `.worktrees/swarm-tennisi` | Зафиксировать readiness как direct-response path, не DOM-intercept guess |
| BetM lane | `.worktrees/swarm-betm` | Снять один live-proof сигнал или подтвердить DiagnosticOnly blocker |
| Betcity lane | `.worktrees/swarm-betcity` | Сузить zero-event regression до noise vs promotion blocker |
| Zenit lane | `.worktrees/swarm-zenit` | Добавить компактный readiness snapshot в board/API truth |
| Baltbet lane | `.worktrees/swarm-baltbet` | Зафиксировать production-ready readiness truth в factory/tests |
| Olimp lane | `.worktrees/swarm-olimp` | Подтвердить live fetch/event-volume truth после re-enable decision |
| Coordinator | read-only | Держит expanded wave 4 board compact, current и без narrative drift |
| Service slot | rotating | `service-bookmaker-status-catalog` в работе; следующий shared slot не поднимать до handoff |

## Service rotation queue

- `.worktrees/swarm-core-safety`
- `.worktrees/swarm-api-operator`
- `.worktrees/swarm-execution-money`
- `.worktrees/swarm-agent-improvement`
- `.worktrees/swarm-ui-operator`
- `.worktrees/swarm-legacy-python`

## Rotation rule

- Если parser worker закончил локальный bounded patch, его слот сразу отдаётся следующему приоритетному worker.
- Если worker упёрся во внешний blocker, он обязан оставить honest handoff: `what works`, `what failed`, `what remains external`.
- Service slot всегда получает задачу, которая повышает общий throughput всей системы, а не локально украшает код.
- Handoff и живая repo-память идут через `docs/memory/*` и `COMPRESSION.md`, а не через длинные чат-сводки.
- Self-driving loop идёт через `docs/memory/AUTONOMY_LOOP.md` и compact idea pool в `docs/memory/IDEA_GENERATOR.md`.
- Активный lane берёт верхний незавершённый task для своей роли из `config/swarm/tasks.json`; worker-proposed follow-up становится backlog/idea seed, пока coordinator не поднимет его в board.
- `dispatch` loop может автоматически refill idle lanes из queued same-lane tasks без ручного claim.

## Current priorities

- Current active wave keeps all bookmaker lanes on the board, with new execution focus on unfinished expansion tasks plus coordinator and one rotating service lane.
- Completed before/at wave start: `winline-stable-feed-proof`, `melbet-bootstrap-evidence-pack`, `betboom-runtime-shape-decision`, `ligastavok-browser-bootstrap-capture`, `service-parser-capability-catalog`, `coordinator-wave-3-board`.
- Active now: `tennisi-readiness-assertion`, `betm-live-proof`, `betcity-regression-truth`, `zenit-add-readiness`, `baltbet-readiness-lock`, `olimp-live-fetch-proof`, `service-bookmaker-status-catalog`, `coordinator-wave-4-board`.
- Generated next queue: `-`.

1. `winline`
2. `melbet`
3. `betboom`
4. `ligastavok`
5. `tennisi`
6. `betm`
7. `betcity`
8. `zenit`
9. `baltbet`
10. `olimp`
11. `service-bookmaker-status-catalog`
12. `coordinator-wave-4-board`
