# Autonomous Swarm

This repo supports parallel development, but only if the swarm stays bounded.

## Principles

- Rust workspace under `crates/` is the mainline.
- Root-level Python is legacy/reference and should not be used as the default execution path.
- Do not run broad smoke tests or browser/network scripts by default.
- Use worktrees so each agent owns an isolated checkout.

## First bootstrap

```powershell
.\bootstrap.ps1 -Quick
```

Optional full validation:

```powershell
.\bootstrap.ps1
```

## Safe command set

Use these as the default autonomous validation loop:

```powershell
git status --short
py -m pytest --collect-only -q
cargo check -p shared
cargo check -p engine
cargo check -p persistence
cargo check -p scanner
cargo check -p parsers
```

When the slice is stable:

```powershell
cargo test -p shared --lib --quiet
cargo test -p engine --lib --quiet
cargo test -p persistence --lib --quiet
cargo test -p scanner --lib --quiet
cargo test -p parsers --lib --quiet
```

## Unsafe by default

Do not include these in unattended loops unless the task explicitly requires them:

- `.\bootstrap.ps1` on a dirty machine with unknown global state
- `cargo test --workspace`
- root `test_*.py` scripts outside `tests/`
- Playwright and browser discovery scripts
- queue runners and artifact-producing scripts
- `cargo run -p fork_hunter_bin` against live credentials

## Worktrees

Dot-source the script so helper functions stay available in the current shell:

```powershell
. .\worktrees.ps1
New-ForkHunterSwarm -Bootstrap
Show-ForkHunterSwarm
```

Suggested standard roles:

- `rust-core`
- `parsers`
- `api-bot`
- `integration`
- `legacy-python`

## Local memory strategy

Keep long-lived project context in compact files instead of relying on broad chat history:

- `DEV_SETUP.md` for environment bootstrap
- `docs/onboarding/AUTONOMOUS_SWARM.md` for agent guardrails
- `desktop-ui/README.md` for frontend loop
- `config.yaml.example` for runtime defaults

