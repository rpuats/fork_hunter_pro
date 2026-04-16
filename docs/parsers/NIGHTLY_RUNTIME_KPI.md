# Nightly Runtime KPI

Nightly runtime diagnostics now have a single entrypoint for strict KPI checks and machine-readable artifacts.

## Command

```powershell
pwsh ./scripts/nightly_runtime_kpi.ps1
```

What it does:

- runs `runtime_parser_diagnostics` with strict nightly exit semantics;
- uses default runtime-only slugs from `crates/parsers/src/diagnostics.rs`;
- writes machine-readable artifacts under `artifacts/nightly/runtime/`.

## Artifacts

- `artifacts/nightly/runtime/runtime_parser_diagnostics_latest.json` - latest full snapshot;
- `artifacts/nightly/runtime/runtime_parser_diagnostics_history.jsonl` - append-only nightly history;
- `artifacts/nightly/runtime/runs/runtime_parser_diagnostics_<timestamp>.json` - immutable per-run snapshot.

## Exit code

- `0` - every requested parser met both KPI thresholds;
- `2` - at least one parser missed live or prematch KPI, or runtime fetch failed.

## Targeted usage

```powershell
pwsh ./scripts/nightly_runtime_kpi.ps1 -Slugs winline,betcity
```

Direct binary usage remains available:

```powershell
cargo run -p parsers --bin runtime_parser_diagnostics -- --help
```
