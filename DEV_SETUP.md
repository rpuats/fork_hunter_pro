# Dev Setup

## Current shape

This repo contains two layers:

1. **Rust workspace** in `crates/` — main direction
2. **Legacy Python stack** in root folders — keep for reference/tools/tests until fully retired

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

Optional tools installed by bootstrap when requested:
- `cargo-nextest`
- `cargo-watch`
- Codex CLI
- Claude Code CLI

## Recommended workflow

### Rust mainline
- `cargo check`
- `cargo test --workspace`
- `cargo run -p fork_hunter_bin`

### Legacy Python
- `py -m pip install -r requirements.txt`
- run individual scripts/tests only as needed

## Agent workflow

Use git worktrees instead of making several agents fight in one folder.

> Note: git worktrees require the repo to have at least one commit. On a fresh/unborn repo, create the initial local commit first.

### Create worktrees
```powershell
.\worktrees.ps1
New-AgentWorktree -Name rust-core -Bootstrap
New-AgentWorktree -Name parsers -Bootstrap
New-AgentWorktree -Name api-bot -Bootstrap
New-AgentWorktree -Name integration -Bootstrap
```

### Initialize / update a worker brief
```powershell
Initialize-AgentWorkspace -Name parsers -Task "Stabilize parsers crate and validate parser tests"
```

Suggested split:
- `rust-core` — `shared`, `engine`, `scanner`, `persistence`
- `parsers` — bookmaker parsers only
- `api-bot` — api/bot/web integration
- `integration` — final wiring, smoke tests, docs

## Docs to read
- `OPENCLAW_WORKFLOW.md`
- `README.md`
- `Cargo.toml`

## Notes
- Root repo is noisy and contains many generated/debug artifacts.
- Actual repo shape has drifted away from historical Python-first docs.
- Do not delete legacy Python outright until migration boundaries are documented.
- Prefer focused Rust changes plus narrow validation in each worktree.
