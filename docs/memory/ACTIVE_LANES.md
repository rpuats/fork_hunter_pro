# Active Lanes

## Runtime constraint

Current practical session limit: `6` live agents.

## Persistent lane model

1. `winline lane`
2. `melbet lane`
3. `betboom lane`
4. `ligastavok lane`
5. `coordinator lane`
6. `service lane`

## Worktree map

- `.worktrees/swarm-winline`
- `.worktrees/swarm-melbet`
- `.worktrees/swarm-betboom`
- `.worktrees/swarm-ligastavok`
- `.worktrees/swarm-core-safety`
- `.worktrees/swarm-api-operator`
- `.worktrees/swarm-ui-operator`
- `.worktrees/swarm-execution-money`
- `.worktrees/swarm-agent-improvement`
- `.worktrees/swarm-legacy-python`

## Service lane rotation

Priority order:

1. `swarm-core-safety`
2. `swarm-api-operator`
3. `swarm-execution-money`
4. `swarm-agent-improvement`
5. `swarm-ui-operator`
6. `swarm-legacy-python`

## Rotation rule

- Keep the 4 bookmaker lanes hot.
- Coordinator stays read-only and reassigns next bounded tasks.
- Service lane always takes the highest-throughput shared improvement next.

## Current wave

- Active: `swarm-winline`, `swarm-melbet`, `swarm-betboom`, `swarm-ligastavok`, `coordinator`, `service`
- Service slot current task: `-`
- Next queue: `swarm-api-operator`, `swarm-ui-operator`, `swarm-legacy-python`
