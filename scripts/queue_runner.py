from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections import OrderedDict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, cast


REPO_ROOT = Path(__file__).resolve().parent.parent

stdout_reconfigure = getattr(sys.stdout, "reconfigure", None)
if callable(stdout_reconfigure):
    stdout_reconfigure(encoding="utf-8", errors="replace")
stderr_reconfigure = getattr(sys.stderr, "reconfigure", None)
if callable(stderr_reconfigure):
    stderr_reconfigure(encoding="utf-8", errors="replace")


def iso_utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def resolve_repo_path(raw: str | None) -> Path:
    if not raw:
        return REPO_ROOT
    path = Path(raw)
    if path.is_absolute():
        return path.resolve()
    return (REPO_ROOT / path).resolve()


def write_json(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def get_latest_run_dir(run_root: Path) -> Path | None:
    dirs = [path for path in run_root.iterdir() if path.is_dir()]
    if not dirs:
        return None
    return sorted(dirs, key=lambda item: item.stat().st_mtime, reverse=True)[0]


def persisted_job(job_state: dict) -> dict:
    return {key: value for key, value in job_state.items() if key != "process"}


def normalize_job(job: dict, defaults: dict, run_dir: Path) -> dict:
    job_id = job["id"]
    job_dir = run_dir / job_id
    job_dir.mkdir(parents=True, exist_ok=True)
    return {
        "id": job_id,
        "label": job.get("label"),
        "shell": job.get("shell", defaults["shell"]),
        "workingDirectory": str(resolve_repo_path(job.get("workingDirectory", defaults["workingDirectory"]))),
        "timeoutSec": int(job.get("timeoutSec", defaults["timeoutSec"])),
        "command": job["command"],
        "status": "queued",
        "createdAt": iso_utc_now(),
        "startedAt": None,
        "finishedAt": None,
        "durationSec": None,
        "exitCode": None,
        "pid": None,
        "artifactDir": str(job_dir),
        "stdoutPath": str(job_dir / "stdout.log"),
        "stderrPath": str(job_dir / "stderr.log"),
        "resultPath": str(job_dir / "result.json"),
        "note": None,
    }


def shell_command(shell: str, command: str) -> list[str]:
    if shell == "powershell":
        return ["pwsh", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", command]
    if shell == "cmd":
        return ["cmd.exe", "/c", command]
    raise ValueError(f"Unsupported shell '{shell}'")


def save_job_result(job_state: dict) -> None:
    write_json(Path(job_state["resultPath"]), persisted_job(job_state))


def run_snapshot(run_state: dict) -> dict:
    jobs = [persisted_job(run_state["jobs"][job_id]) for job_id in run_state["jobOrder"]]
    counts = {
        "queued": sum(1 for job in jobs if job["status"] == "queued"),
        "running": sum(1 for job in jobs if job["status"] == "running"),
        "succeeded": sum(1 for job in jobs if job["status"] == "succeeded"),
        "failed": sum(1 for job in jobs if job["status"] == "failed"),
        "timedOut": sum(1 for job in jobs if job["status"] == "timed_out"),
    }
    return {
        "runId": run_state["runId"],
        "queuePath": run_state["queuePath"],
        "runRoot": run_state["runRoot"],
        "maxParallel": run_state["maxParallel"],
        "pollIntervalMs": run_state["pollIntervalMs"],
        "startedAt": run_state["startedAt"],
        "finishedAt": run_state["finishedAt"],
        "status": run_state["status"],
        "counts": counts,
        "jobs": jobs,
    }


def save_run_state(run_state: dict, run_dir: Path) -> None:
    snapshot = run_snapshot(run_state)
    write_json(run_dir / "state.json", snapshot)
    write_json(run_dir / "summary.json", snapshot["counts"])


def start_job(job_state: dict) -> None:
    command = shell_command(job_state["shell"], job_state["command"])
    stdout_path = Path(job_state["stdoutPath"])
    stderr_path = Path(job_state["stderrPath"])
    stdout_path.write_text("", encoding="utf-8")
    stderr_path.write_text("", encoding="utf-8")
    stdout_handle = stdout_path.open("w", encoding="utf-8")
    stderr_handle = stderr_path.open("w", encoding="utf-8")
    process = subprocess.Popen(
        command,
        cwd=job_state["workingDirectory"],
        stdout=stdout_handle,
        stderr=stderr_handle,
        text=True,
    )
    job_state["status"] = "running"
    job_state["startedAt"] = iso_utc_now()
    job_state["pid"] = process.pid
    job_state["deadlineAt"] = time.time() + job_state["timeoutSec"]
    job_state["process"] = {
        "proc": process,
        "stdout": stdout_handle,
        "stderr": stderr_handle,
    }
    job_state["note"] = "started"
    save_job_result(job_state)


def close_job_handles(job_state: dict) -> None:
    process_info = job_state.pop("process", None)
    if process_info:
        process_info["stdout"].close()
        process_info["stderr"].close()
    job_state.pop("deadlineAt", None)


def finish_job(job_state: dict, status: str, exit_code: int | None, note: str) -> None:
    finished = datetime.now(timezone.utc)
    started = datetime.fromisoformat(job_state["startedAt"]) if job_state["startedAt"] else finished
    job_state["status"] = status
    job_state["finishedAt"] = finished.isoformat()
    job_state["durationSec"] = round((finished - started).total_seconds(), 3)
    job_state["exitCode"] = exit_code
    job_state["note"] = note
    close_job_handles(job_state)
    save_job_result(job_state)


def run_queue(args: argparse.Namespace) -> int:
    queue_path = resolve_repo_path(args.queue_path)
    run_root = resolve_repo_path(args.run_root)
    run_root.mkdir(parents=True, exist_ok=True)

    queue = cast(dict[str, Any], read_json(queue_path))
    defaults = cast(dict[str, Any], queue.get("defaults", {}))
    defaults = {
        "shell": defaults.get("shell", "powershell"),
        "workingDirectory": defaults.get("workingDirectory", "."),
        "timeoutSec": int(defaults.get("timeoutSec", 120)),
    }
    max_parallel = args.max_parallel if args.max_parallel is not None else int(queue.get("maxParallel", 2))
    if max_parallel < 1:
        raise ValueError("maxParallel must be >= 1")

    run_id = args.run_id or datetime.now().strftime("run-%Y%m%d-%H%M%S")
    run_dir = run_root / run_id
    run_dir.mkdir(parents=True, exist_ok=True)

    jobs = OrderedDict()
    for raw_job in queue.get("jobs", []):
        job_id = raw_job.get("id")
        command = raw_job.get("command")
        if not job_id or not command:
            raise ValueError("Each job must have id and command")
        if job_id in jobs:
            raise ValueError(f"Duplicate job id '{job_id}'")
        jobs[job_id] = normalize_job(raw_job, defaults, run_dir)

    run_state = {
        "runId": run_id,
        "queuePath": str(queue_path),
        "runRoot": str(run_root),
        "maxParallel": max_parallel,
        "pollIntervalMs": args.poll_interval_ms,
        "startedAt": iso_utc_now(),
        "finishedAt": None,
        "status": "running",
        "jobOrder": list(jobs.keys()),
        "jobs": jobs,
    }
    save_run_state(run_state, run_dir)

    next_index = 0
    while True:
        running_jobs = [job for job in jobs.values() if job["status"] == "running"]
        while len(running_jobs) < max_parallel and next_index < len(run_state["jobOrder"]):
            job_id = run_state["jobOrder"][next_index]
            next_index += 1
            start_job(jobs[job_id])
            save_run_state(run_state, run_dir)
            running_jobs = [job for job in jobs.values() if job["status"] == "running"]

        now = time.time()
        for job in [item for item in jobs.values() if item["status"] == "running"]:
            proc = job["process"]["proc"]
            exit_code = proc.poll()
            if exit_code is not None:
                status = "succeeded" if exit_code == 0 else "failed"
                finish_job(job, status, exit_code, "process exited")
                continue
            if now >= job["deadlineAt"]:
                proc.kill()
                proc.wait(timeout=5)
                finish_job(job, "timed_out", None, "timed out and terminated")

        save_run_state(run_state, run_dir)
        if all(job["status"] not in {"queued", "running"} for job in jobs.values()):
            break
        time.sleep(args.poll_interval_ms / 1000.0)

    failed = sum(1 for job in jobs.values() if job["status"] == "failed")
    timed_out = sum(1 for job in jobs.values() if job["status"] == "timed_out")
    succeeded = sum(1 for job in jobs.values() if job["status"] == "succeeded")
    run_state["finishedAt"] = iso_utc_now()
    run_state["status"] = "completed_with_errors" if failed or timed_out else "completed"
    save_run_state(run_state, run_dir)

    print(f"Queue run complete: {run_id}")
    print(f"Artifacts       : {run_dir}")
    print(f"Succeeded       : {succeeded}")
    print(f"Failed          : {failed}")
    print(f"Timed out       : {timed_out}")
    return 1 if failed or timed_out else 0


def show_status(args: argparse.Namespace) -> int:
    run_root = resolve_repo_path(args.run_root)
    run_dir = run_root / args.run_id if args.run_id else get_latest_run_dir(run_root)
    if run_dir is None or not run_dir.exists():
        raise FileNotFoundError("No queue runs found")
    state = cast(dict[str, Any], read_json(run_dir / "state.json"))
    counts = state["counts"]
    print(f"runId       : {state['runId']}")
    print(f"status      : {state['status']}")
    print(f"queuePath   : {state['queuePath']}")
    print(f"maxParallel : {state['maxParallel']}")
    print(
        "counts      : queued={queued} running={running} succeeded={succeeded} failed={failed} timed_out={timedOut}".format(
            **counts
        )
    )
    print("jobs:")
    for job in state["jobs"]:
        exit_code = "n/a" if job["exitCode"] is None else job["exitCode"]
        print(f"- {job['id']}: {job['status']} (timeout={job['timeoutSec']}s, exit={exit_code})")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Local bounded queue runner")
    parser.add_argument("--action", choices=["run", "status"], default="run")
    parser.add_argument("--queue-path", default="artifacts/queue/example-queue.json")
    parser.add_argument("--run-root", default="artifacts/queue/runs")
    parser.add_argument("--run-id")
    parser.add_argument("--max-parallel", type=int)
    parser.add_argument("--poll-interval-ms", type=int, default=500)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.action == "status":
        return show_status(args)
    return run_queue(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"queue_runner error: {exc}", file=sys.stderr)
        raise
