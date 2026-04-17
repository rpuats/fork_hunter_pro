# Kilo Fast Path for `fork_hunter_pro`

This repo is easiest to drive in Kilo when the session follows the actual project shape: Rust mainline in `crates/`, legacy Python at the root only when comparison or migration work is required, and focused validation instead of full-repo churn.

## What the environment audit means in practice

- the repository root is noisy, so broad file searches often surface logs and debug artifacts before source files;
- the active product path is the Rust workspace, not the historical Python scripts in the root;
- the safest fast loop is PowerShell bootstrap, a crate-scoped change, then a slice-specific cargo check/test;
- worktree-based parallelism is preferred for larger tasks because the repo already documents role splits.

## Recommended Kilo entrypoints

Use these first:

- `/bootstrap-fast` - provision `.env`, install what is needed, and get to a usable local state quickly;
- `/bookmaker-memory-preflight <slug>` - read the current bookmaker lessons, blockers, and reusable patterns before a bounded parser task;
- `/bounded-rust-return <goal>` - keep short bounded Rust tasks on a single slice and return immediately on the first real blocker;
- `/rust-slice-check rust-core` - validate core crates only;
- `/rust-slice-check parsers` - validate parser-only work;
- `/rust-slice-check api-bot` - validate runtime/API wiring without paying for full workspace tests;
- `/nightly-runtime-kpi` - run strict parser KPI diagnostics and inspect artifact output.

## Recommended agent

Use `@fork-hunter-rust-mainline` when the task is in the Rust workspace, needs targeted validation, or risks getting lost in root-level legacy noise.

## Default operating loop

1. Run `/bootstrap-fast` on fresh clones or stale environments.
2. For bookmaker work, run `/bookmaker-memory-preflight <slug>` before implementation so the session starts from the latest local lessons and blockers.
3. For short bounded asks, start with `/bounded-rust-return <goal>` so the session stays inside one slice and fails fast.
4. Keep implementation work inside the narrowest crate or role boundary.
5. Run `/rust-slice-check <role>` before expanding to broader validation.
6. Use `/nightly-runtime-kpi` only for parser quality gates or nightly regression checks.
7. After a bounded bookmaker task, capture the new lesson with `/bookmaker-memory-capture <slug> <kind> <summary>`.
8. Escalate to workspace-wide checks only after the focused slice is green.

## Blocker-first return rule

- if the first blocker is environmental, missing context, or outside the chosen slice, stop and report it immediately;
- return the smallest proof of progress before the blocker, such as one edit, one cargo command, or one inspected crate boundary;
- do not turn a bounded task into broad repo spelunking just to avoid returning blocked.

## Rolling agent usage

- start with `@fork-hunter-rust-mainline` for nearly all bounded implementation work in `crates/`;
- keep the same agent while the task stays in one slice; only roll forward when the blocker clearly moves to another role boundary;
- when rolling forward, name the next slice explicitly in the handoff, for example `parsers` for scraper regressions or `integration` for cross-crate validation.

## Role map

- `rust-core` - `crates/shared`, `crates/engine`, `crates/scanner`, `crates/persistence`
- `parsers` - `crates/parsers` and parser-focused fixtures/tests
- `api-bot` - `crates/api`, `crates/bot`, `crates/fork_hunter_bin`
- `integration` - cross-crate wiring, smoke checks, final docs polish
- `legacy-python` - root scripts and `tests/` only for migration/reference work

## Guardrails

- do not treat root Python files as the default edit surface;
- do not judge the task by unrelated root artifacts or old logs;
- do not jump to `cargo test --workspace` unless the change scope justifies it;
- do use worktrees for parallel agent work, following `OPENCLAW_WORKFLOW.md`.
