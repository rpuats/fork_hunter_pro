# Bookmaker Learning Loop

This repo now keeps bookmaker-specific bounded-task memory in `artifacts/learning/bookmaker_memory.json`.

## Goal

- reuse repo-learned lessons before the next bounded task;
- keep blockers honest instead of rediscovering them from logs;
- capture one small reusable lesson after each bounded task.

## Memory format

The file stores one object per bookmaker with three compact buckets:

- `lessons` - what became true or clearer;
- `blockers` - current honest blockers plus the next bounded move;
- `successPatterns` - approaches worth repeating.

Each note should stay short and evidence-backed.

Required fields:

- `id`
- `kind`
- `summary`
- `evidence`

Use these extra fields when relevant:

- `useWhen` for lessons
- `nextBoundedMove` for blockers
- `repeatFor` for success patterns

## Pre-task loop

Before any bounded task on `winline`, `betboom`, `melbet`, or `ligastavok`:

1. Run `/bookmaker-memory-preflight <slug>`.
2. Read only the target bookmaker entry plus cited evidence paths if needed.
3. Restate the task with the remembered blocker/success pattern in mind.
4. Keep the implementation bounded to the smallest move that changes truth, readiness, or feed-path confidence.

## Post-task loop

After the bounded task:

1. Run `/bookmaker-memory-capture <slug> <lesson|blocker|success_pattern> <summary>`.
2. Update `artifacts/learning/bookmaker_memory.json` with one small evidence-backed note.
3. Prefer appending a new note over rewriting history unless the old note is now false.
4. If a blocker changed, update the bookmaker `status` and `nextBoundedMove` too.

## Writing rules

- Keep each `summary` to one or two sentences.
- Reference repo evidence, not chat memory.
- Record honest blockers, not aspirational guesses.
- Prefer patterns that can guide the next bounded task in under a minute.

## Current seeded targets

- `winline` - bounded runtime path is clearer; stable feed path is still unresolved.
- `betboom` - compact runtime fallback improved; real feed path still needs confirmation.
- `melbet` - browser-interception path remains the right default; blocker is tighter but unresolved.
- `ligastavok` - anti-bot/session bootstrap is still external; readiness taxonomy is already valuable and should be preserved.
