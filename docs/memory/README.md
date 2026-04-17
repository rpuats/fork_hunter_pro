# Memory Bank

Compact repo-native memory for long-running AI development.

## Purpose

This folder exists to reduce token waste and keep active engineering truth in short files instead of long chat history.

## Files

- `PROJECT_STATE.md` — what the project is, where it stands, what matters now
- `BOOKMAKER_BOARD.md` — bookmaker-by-bookmaker status and blockers
- `ACTIVE_LANES.md` — current swarm lanes, worktrees, queue rotation
- `DECISIONS.md` — accepted architectural and workflow decisions
- `BACKLOG.md` — compact prioritized next work
- `HANDOFF_TEMPLATE.md` — required shape for worker handoff

## Rules

- Keep entries short and current.
- Update only when something materially changes.
- Prefer facts, blockers, and decisions over narrative.
- Do not duplicate large logs or test output here.

## Read order

1. `PROJECT_STATE.md`
2. `BOOKMAKER_BOARD.md`
3. `ACTIVE_LANES.md`
4. `DECISIONS.md`
5. `BACKLOG.md`

## Control plane

- `config/swarm/lanes.json` — persistent lane manifest
- `config/swarm/tasks.json` — bounded task seed/queue
- `scripts/swarm_control.py` — claim/complete/status for multi-session work
- `docs/onboarding/MULTI_SESSION_SWARM.md` — operating guide for independent Codex sessions

## Current focus

- Keep the 4 bookmaker lanes, coordinator, and rotating service lane in sync with `docs/onboarding/SWARM_STATUS.md`.
- Use `config/swarm/tasks.json` for the compact task board.
