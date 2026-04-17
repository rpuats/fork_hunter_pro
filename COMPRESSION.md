# COMPRESSION.md

Context compression policy for long-running AI work in `fork_hunter_pro`.

## Goals

- Reduce token waste during long autonomous sessions.
- Preserve active engineering truth.
- Prevent drift caused by verbose logs, repeated summaries, and stale exploration.

## Triggers

### Incremental compress

Run when any of these is true:

- context feels crowded or repetitive
- a worker finished a bounded task
- tool output is long and mostly diagnostic
- more than 3 unresolved side threads exist

### Full compress

Run when any of these is true:

- handoff between waves of agents
- major scope switch: parsers -> operator -> execution
- a session starts losing earlier decisions
- branch/worktree state has materially changed

## Preserve always

- active user request
- current branch and worktree map
- live blockers
- exact file paths touched in the current wave
- last accepted architectural decisions
- validation results that actually changed confidence
- open risks that can change implementation

## Compress aggressively

- repeated progress updates
- raw shell noise
- long directory listings
- exploratory dead ends
- duplicate explanations of project architecture
- repeated bookmaker status summaries
- verbose test output when the pass/fail/result is already known

## Discard by default

- motivational filler
- repeated acknowledgements
- obsolete intermediate plans
- stale hypotheses disproven by later diagnostics
- logs that do not affect current next steps

## Output format

Every compression result should fit this shape:

1. Objective
2. Current state
3. Current wave and worktrees
4. Changed files
5. Validation
6. Blockers
7. Next tasks

## Swarm-specific rules

- Each worker reports only:
  - what changed
  - how to verify
  - what remains blocked
- Coordinator stores only current wave, next wave, and slot rotation.
- Product/ideas analysis becomes backlog items, not long essays.
- Parser work must preserve bookmaker-specific blockers separately.

## Verification

After compression, the compressed state must still answer:

- what the user asked for
- which 6 lanes are active
- which worktrees they own
- what changed in this wave
- what to do next without reopening large logs

If not, compression failed and should be redone.
