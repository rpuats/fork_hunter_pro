# Active Lanes

## Runtime constraint

Current practical per-session limit: `6` live agents.

Expanded swarm uses independent worktrees/sessions, so the board may keep more than six lanes active overall.

## Persistent lane model

1. `winline lane`
2. `melbet lane`
3. `betboom lane`
4. `ligastavok lane`
5. `tennisi lane`
6. `betm lane`
7. `betcity lane`
8. `zenit lane`
9. `baltbet lane`
10. `olimp lane`
11. `coordinator lane`
12. `service lane`

## Worktree map

- `.worktrees/swarm-winline`
- `.worktrees/swarm-melbet`
- `.worktrees/swarm-betboom`
- `.worktrees/swarm-ligastavok`
- `.worktrees/swarm-tennisi`
- `.worktrees/swarm-betm`
- `.worktrees/swarm-betcity`
- `.worktrees/swarm-zenit`
- `.worktrees/swarm-baltbet`
- `.worktrees/swarm-olimp`
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

- Keep finished bookmaker lanes available for promotion follow-up and idle slots aimed at the highest-priority unfinished bookmaker task.
- Coordinator stays read-only and reassigns next bounded tasks.
- Service lane always takes the highest-throughput shared improvement next.
- Each lane pulls the highest-priority unfinished task for its lane from `config/swarm/tasks.json`; validated follow-ups downgrade to backlog/idea seeds until coordinator promotes them.
- Lanes may self-assign the next bounded task only inside their owned slice and only after a narrow validation-backed handoff.

## Current wave

- Active: `swarm-winline`, `swarm-melbet`, `swarm-betboom`, `swarm-ligastavok`, `swarm-tennisi`, `swarm-betm`, `swarm-betcity`, `swarm-zenit`, `swarm-baltbet`, `swarm-olimp`, `coordinator`, `service`
- Service slot current task: `service-bookmaker-status-catalog`
- Coordinator active task: `coordinator-wave-4-board`
- Generated next queue: `-`
- Autonomy protocol: `docs/memory/AUTONOMY_LOOP.md`
- Dispatcher: `python .\scripts\swarm_control.py dispatch --iterations 12 --interval-secs 10`
