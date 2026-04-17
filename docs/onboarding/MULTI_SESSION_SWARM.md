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
- `AGENT_SWARM.md` — lane model and guardrails
- `docs/onboarding/SWARM_STATUS.md` — current wave and rotation
- `config/swarm/lanes.json` — lane manifest
- `config/swarm/tasks.json` — seeded bounded tasks
- `scripts/swarm_control.py` — minimal control plane

## Current shape

- Active wave: `winline`, `melbet`, `betboom`, `ligastavok`, `coordinator`, `service`
- Service slot current task: `-`
- Shared queue: `api-operator`, `ui-operator`, `legacy-python`
- Board source of truth: `config/swarm/tasks.json` plus `docs/memory/*`

## Quick start

From repo root:

```powershell
python .\scripts\swarm_control.py ensure-state
python .\scripts\swarm_control.py status
```

To claim the next task for a lane:

```powershell
python .\scripts\swarm_control.py claim-next --lane winline
python .\scripts\swarm_control.py claim-next --lane melbet
python .\scripts\swarm_control.py claim-next --lane betboom
python .\scripts\swarm_control.py claim-next --lane ligastavok
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
5. coordinator in main checkout
6. service lane in main checkout or its target `swarm-*` worktree

## Operating rule

- Each session reads `docs/memory/README.md` first.
- Each session claims one bounded task.
- Each session writes short handoff only.
- Coordinator updates memory/board, not large narratives.
