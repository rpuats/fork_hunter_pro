# Decisions

## Accepted

### Rust is mainline

- Rust workspace under `crates/` is the production path.
- Root-level Python is legacy, diagnostic, migration, or comparison support.

### Hard parser work comes first

- Current highest ROI is `winline`, `melbet`, `betboom`, `ligastavok`.
- Do not dilute focus with low-value polish while these remain unresolved.

### Truthfulness over fake success

- Better to expose honest blockers/readiness than claim parser health with weak payloads.
- Diagnostics, caps, staleness, validators, and bounded failure are product features.

### Execution stays safety-first

- No unsafe real-money rollout before stronger approval, drift checks, and consistency layers.

### Memory lives in repo

- Compact repo files are preferred over long chat recall.
- Update the memory bank only when facts or priorities materially change.

### Swarm is lane-based, not hierarchy-based

- Live thread limit makes fake subagent trees a bad trade.
- Throughput comes from stable lanes, bounded tasks, and fast handoff.
