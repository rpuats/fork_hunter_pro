# Project State

## What this is

`Fork Hunter Pro` is a Rust-first arbitrage operations platform for bookmaker odds collection, matching, surebet detection, operator visibility, bankroll readiness, execution groundwork, and freebet planning.

## Main layers

- `parser fleet`
- `scanner core`
- `operator surfaces`
- `bankroll/funding`
- `execution/freebet`

## Current maturity

Strong enough to treat as a serious platform, not an experiment.

Already strong:

- scanner core
- parser infrastructure
- runtime safety
- API/UI/operator surfaces
- 3 confirmed strong-pass bookmakers
- execution/freebet foundations

Not finished:

- remaining hard parsers
- safe semi-auto execution
- full freebet engine
- stronger bankroll intelligence

## Current highest-ROI focus

1. Finish the remaining hard parser promotion steps after the new bounded patches
2. Improve operator readiness/triage surfaces
3. Strengthen execution safety before any real-money automation
4. Keep runtime truthfulness improvements compact and cumulative
