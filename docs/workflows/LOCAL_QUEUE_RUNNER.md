# Local Queue Runner

`scripts/queue_runner.ps1` is a local bounded workflow runner for short project jobs. It does not depend on Kilo subagents and is designed for Windows-first repo automation.

## What it does

- reads a queue manifest from JSON;
- starts up to `maxParallel` jobs at once;
- advances to the next queued job as soon as a slot frees up;
- enforces per-job timeouts;
- writes aggregate and per-job artifacts under `artifacts/queue/runs/<run-id>/`.

## Queue schema

```json
{
  "version": 1,
  "maxParallel": 2,
  "defaults": {
    "shell": "powershell",
    "workingDirectory": ".",
    "timeoutSec": 120
  },
  "jobs": [
    {
      "id": "job-id",
      "label": "optional human label",
      "command": "./scripts/mock_queue_job.ps1 -Name demo -DelaySec 1",
      "timeoutSec": 30,
      "workingDirectory": ".",
      "shell": "powershell"
    }
  ]
}
```

Supported shells:

- `powershell` - runs with `pwsh -NoLogo -NoProfile -ExecutionPolicy Bypass -Command ...`;
- `cmd` - runs with `cmd.exe /c ...`.

## Run it

```powershell
pwsh ./scripts/queue_runner.ps1 -Action run -QueuePath artifacts/queue/example-queue.json
```

Optional overrides:

```powershell
pwsh ./scripts/queue_runner.ps1 -Action run -QueuePath artifacts/queue/example-queue.json -MaxParallel 3 -PollIntervalMs 250
```

## Inspect status

Latest run:

```powershell
pwsh ./scripts/queue_runner.ps1 -Action status
```

Specific run:

```powershell
pwsh ./scripts/queue_runner.ps1 -Action status -RunId run-20260414-201500
```

## Artifacts

Each run writes:

- `artifacts/queue/runs/<run-id>/state.json` - full queue snapshot;
- `artifacts/queue/runs/<run-id>/summary.json` - aggregate counts;
- `artifacts/queue/runs/<run-id>/<job-id>/stdout.log` - captured standard output;
- `artifacts/queue/runs/<run-id>/<job-id>/stderr.log` - captured standard error;
- `artifacts/queue/runs/<run-id>/<job-id>/result.json` - per-job metadata.

## Example manifest

`artifacts/queue/example-queue.json` includes:

- two successful mock jobs;
- one job that times out;
- one job that returns a non-zero exit code.

This gives a fast smoke test for scheduler transitions, timeout handling, and artifact writing.
