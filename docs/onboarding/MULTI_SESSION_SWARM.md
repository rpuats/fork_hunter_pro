# Multi-Session Swarm

This repo can run a stronger swarm by using multiple independent Codex sessions, each pinned to its own worktree, while sharing a compact repo-native memory and task board.

## Why

One session has a live-agent limit. Multiple independent sessions avoid that bottleneck as long as they share:

- isolated worktrees
- compact memory files
- a single lane/task board
- short handoff format

## Core pieces

- `docs/memory/*` — compact shared memory
- `COMPRESSION.md` — token budget policy
- `docs/memory/AUTONOMY_LOOP.md` — self-driving lane protocol
- `docs/memory/IDEA_GENERATOR.md` — bounded next-task pool
- `AGENT_SWARM.md` — lane model and guardrails
- `docs/onboarding/SWARM_STATUS.md` — current wave and rotation
- `config/swarm/lanes.json` — lane manifest
- `config/swarm/tasks.json` — seeded bounded tasks
- `scripts/swarm_control.py` — minimal control plane
- `python .\scripts\swarm_control.py dispatch` — bounded dispatcher loop for auto-claiming idle lanes

## Current shape

- Active wave: `winline`, `melbet`, `betboom`, `ligastavok`, `tennisi`, `betm`, `betcity`, `zenit`, `baltbet`, `olimp`, `coordinator`, `service`
- Service slot current task: `service-bookmaker-status-catalog`
- Shared queue: `api-operator`, `execution-money`, `agent-improvement`, `ui-operator`, `legacy-python`
- Board source of truth: `config/swarm/tasks.json` plus `docs/memory/*`

## Quick start

From repo root:

```powershell
python .\scripts\swarm_control.py ensure-state
python .\scripts\swarm_control.py status
python .\scripts\swarm_control.py dispatch --iterations 12 --interval-secs 10
```

To claim the next task for a lane:

```powershell
python .\scripts\swarm_control.py claim-next --lane winline
python .\scripts\swarm_control.py claim-next --lane melbet
python .\scripts\swarm_control.py claim-next --lane betboom
python .\scripts\swarm_control.py claim-next --lane ligastavok
python .\scripts\swarm_control.py claim-next --lane tennisi
python .\scripts\swarm_control.py claim-next --lane betm
python .\scripts\swarm_control.py claim-next --lane betcity
python .\scripts\swarm_control.py claim-next --lane zenit
python .\scripts\swarm_control.py claim-next --lane baltbet
python .\scripts\swarm_control.py claim-next --lane olimp
python .\scripts\swarm_control.py claim-next --lane service
```

When a bounded task is done:

```powershell
python .\scripts\swarm_control.py complete --task-id winline-fanout-bounds --note "bounded fanout path improved"
```

## Session layout

Recommended independent sessions:

1. `swarm-winline`
2. `swarm-melbet`
3. `swarm-betboom`
4. `swarm-ligastavok`
5. `swarm-tennisi`
6. `swarm-betm`
7. `swarm-betcity`
8. `swarm-zenit`
9. `swarm-baltbet`
10. `swarm-olimp`
11. coordinator in main checkout
12. service lane in main checkout or its target `swarm-*` worktree

## Operating rule

- Each session reads `docs/memory/README.md` first.
- Each session claims one bounded task.
- Each session writes short handoff only.
- Coordinator updates memory/board, not large narratives.
- If a lane finishes cleanly, it may propose one same-slice next task; coordinator decides whether to promote it to the board.
- Dispatcher may keep idle lanes hot by auto-claiming queued same-lane tasks on a bounded timer loop.
