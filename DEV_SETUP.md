# Dev Setup

## Repo shape right now

This repository is intentionally mixed:

1. **Rust workspace in `crates/`** — mainline for ongoing development
2. **Legacy Python in the repo root** — reference implementation, ad-hoc tooling, old tests, and migration scaffolding

The Rust side is not just `shared/engine/parsers`. The active workspace currently includes:

- `shared`
- `engine`
- `persistence`
- `scanner`
- `api`
- `bot`
- `parsers`
- `fork_hunter_bin`
- `corridor_scanner`
- `express_forks`
- `bankroll_manager`
- `bonus_hunter`
- `auto_betting`
- the root package (`.`)

## Recommended bootstrap

### Fast path
```powershell
.\bootstrap.ps1 -Quick
```

### Full local validation path
```powershell
.\bootstrap.ps1
```

### Optional extra tooling
```powershell
.\bootstrap.ps1 -InstallOptionalTools
```

### Useful switches
```powershell
.\bootstrap.ps1 -SkipPython
.\bootstrap.ps1 -SkipRust
```

Bootstrap behavior:
- copies `.env.example` to `.env` if needed
- installs Python requirements for the legacy/reference toolchain when Python is available
- runs `cargo check`
- on non-quick runs, executes focused Rust tests for `shared`, `engine`, `parsers`, `scanner`, and `persistence`
- installs Codex CLI and Claude Code CLI when `npm` is available
- installs `cargo-nextest` and `cargo-watch` only with `-InstallOptionalTools`

## Recommended workflow

### Rust mainline
```powershell
cargo check
cargo test --workspace
cargo run -p fork_hunter_bin
```

Use targeted crate validation when possible for faster agent loops.

### Legacy Python
```powershell
py -m pip install -r requirements.txt
pytest tests
```

Use root Python scripts/tests only when they are part of migration verification or parser behavior comparison.

## Agent workflow

Use git worktrees instead of letting multiple agents edit one checkout.

> Note: git worktrees require the repo to have at least one commit. This repo already has history, but the guard remains useful for fresh clones or copied snapshots.

### Create worktrees
```powershell
.\worktrees.ps1
New-ForkHunterSwarm -Bootstrap
```

Equivalent manual flow:

```powershell
New-AgentWorktree -Name rust-core -Bootstrap
New-AgentWorktree -Name parsers -Bootstrap
New-AgentWorktree -Name api-bot -Bootstrap
New-AgentWorktree -Name integration -Bootstrap
New-AgentWorktree -Name legacy-python -Bootstrap
```

### Initialize / update a worker brief
```powershell
Initialize-AgentWorkspace -Name parsers -Task "Stabilize parsers crate and validate parser tests"
```

Suggested split:
- `rust-core` — `shared`, `engine`, `scanner`, `persistence`
- `parsers` — bookmaker parsers only
- `api-bot` — `api`, `bot`, `fork_hunter_bin`
- `integration` — workspace-wide validation, docs, final wiring
- `legacy-python` — root Python scripts/tests only when explicitly needed

## Docs to read first
- `OPENCLAW_WORKFLOW.md`
- `OPENCLAW_STACK.md`
- `README.md` (historical; not fully aligned with the Rust workspace yet)
- `Cargo.toml`

## Notes
- Root repo is noisy and contains many generated/debug artifacts.
- Historical Python-first docs still exist; trust `Cargo.toml` and `crates/` for current mainline structure.
- Do not delete legacy Python outright until migration boundaries are documented.
- Prefer focused Rust changes plus narrow validation inside each worktree.
