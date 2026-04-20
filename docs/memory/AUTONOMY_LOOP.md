# Autonomy Loop

Compact protocol for self-driving swarm work.

## Goal

Let active lanes keep moving without waiting for a new human prompt.

## Lane loop

1. Read `docs/memory/README.md`, `PROJECT_STATE.md`, `ACTIVE_LANES.md`, `BACKLOG.md`, `HANDOFF_TEMPLATE.md`, and `COMPRESSION.md`.
2. Claim one bounded lane task from `config/swarm/tasks.json`.
3. Work only inside the owned slice/worktree.
4. Validate narrowly.
5. Write a short handoff in the required template shape.
6. If the task is complete, propose the next bounded task in one sentence.
7. If blocked externally, record the blocker honestly and downgrade the lane to the next useful bounded task.

## Self-assignment rule

- A lane may self-assign the next task only if it is:
  - in the same owned slice
  - bounded enough for one session
  - clearly derived from the current blocker or fresh validation
- Otherwise the lane returns control to coordinator.

## Coordinator loop

1. Keep only the current wave, next queue, and blocker map current.
2. Accept worker-proposed next tasks only if they are bounded and ownership-safe.
3. Convert good proposals into short backlog/task-board entries.
4. Prefer replacing blocked work with adjacent high-ROI work instead of waiting idle.
5. Use `docs/memory/IDEA_GENERATOR.md` as compact idea pool — promote bounded ideas to task-board when validated.

## Service loop

- Service lane should prefer changes that improve throughput for all lanes:
  - memory clarity
  - token compression
  - operator triage
  - shared runtime diagnostics
  - handoff quality

## Self-driving board maintenance

1. Read `docs/memory/ACTIVE_LANES.md`, `SWARM_STATUS.md`, and `config/swarm/tasks.json` every session.
2. Keep the 6-lane wave compact: 4 parser workers + coordinator + rotating service.
3. Use `scripts/swarm_control.py status` to verify lane state before making board changes.
4. Only update coordinator-owned docs: `AUTONOMY_LOOP.md`, `IDEA_GENERATOR.md`, `ACTIVE_LANES.md`, `SWARM_STATUS.md`.
5. Service slot always picks highest-throughput improvement (not local polish).
6. Promote worker handoffs to task-board when bounded and ownership-safe.
7. Keep idea pool at ≤10 items — drop stale ideas per `IDEA_GENERATOR.md` drop rule.
8. Use `python .\scripts\swarm_control.py dispatch --iterations N --interval-secs S` when you want the board to keep refilling idle lanes without manual claim commands.

## Stop conditions

- external auth/anti-bot wall
- no bounded next move remains
- ownership collision with another lane
- validation cost becomes workspace-wide instead of narrow
