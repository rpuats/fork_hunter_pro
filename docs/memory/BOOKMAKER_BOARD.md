# Bookmaker Board

## Old working core

- `fonbet`
- `pari`
- `bettery`
- `leon`
- `olimpbet`
- `marathon`
- `sportbet`
- `bet24`

## Confirmed strong pass

- `zenit` — confirmed pass, strong runtime KPI
- `betcity` — confirmed pass, strong runtime KPI
- `baltbet` — confirmed pass, strong runtime KPI

## Remaining hard targets

### `winline`

- status: bounded runtime path validated, not pass
- current blocker: production feed path still not confirmed
- latest gain: expensive fanout is cut back and runtime behavior is now more honestly bounded
- desired next move: confirm whether structured payload path can become a stable feed or remain an explicit blocker

### `melbet`

- status: partial, blocker tightened
- current blocker: transport/runtime guardrail after broader live-surface sweep
- latest gain: blocker is narrowed enough that next feed/bootstrap step is obvious without log rereads
- desired next move: production-ready feed path or one tighter transport truth layer

### `betboom`

- status: bounded fallback improved, not production-ready
- current blocker: real runtime feed path still not fully enabled
- latest gain: compact runtime-card fallback now avoids some false empty-result cases
- desired next move: connect guarded Sporthub/runtime groundwork to a real feed path or a more explicit blocker

### `ligastavok`

- status: externally blocked
- current blocker: anti-bot/session bootstrap remains external
- latest gain: readiness now classifies `ready`, `protection_only`, `header_only`, and `bootstrap_unavailable`
- desired next move: stronger browser-assisted bootstrap evidence, not fake bypasses

## Second tier

- `betm`
- `tennisi`
- legacy/discovery-only directions
