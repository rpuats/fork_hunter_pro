from __future__ import annotations

import argparse
import json
import os
import sys
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
CONFIG_ROOT = REPO_ROOT / "config" / "swarm"
STATE_ROOT = REPO_ROOT / "artifacts" / "swarm"
STATE_PATH = STATE_ROOT / "state.json"
LOCK_PATH = STATE_ROOT / "state.lock"
LOCK_TIMEOUT_SECS = 10.0
LOCK_POLL_SECS = 0.05

stdout_reconfigure = getattr(sys.stdout, "reconfigure", None)
if callable(stdout_reconfigure):
    stdout_reconfigure(encoding="utf-8", errors="replace")
stderr_reconfigure = getattr(sys.stderr, "reconfigure", None)
if callable(stderr_reconfigure):
    stderr_reconfigure(encoding="utf-8", errors="replace")


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    temp_path = path.with_suffix(f"{path.suffix}.tmp")
    temp_path.write_text(payload, encoding="utf-8")
    os.replace(temp_path, path)


@contextmanager
def state_lock():
    STATE_ROOT.mkdir(parents=True, exist_ok=True)
    start = time.monotonic()

    while True:
        try:
            fd = os.open(str(LOCK_PATH), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            break
        except FileExistsError:
            if time.monotonic() - start >= LOCK_TIMEOUT_SECS:
                raise SystemExit(f"Timed out waiting for swarm state lock: {LOCK_PATH}")
            time.sleep(LOCK_POLL_SECS)

    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(f"pid={os.getpid()} acquired_at={now_iso()}\n")
        yield
    finally:
        try:
            LOCK_PATH.unlink()
        except FileNotFoundError:
            pass


def load_config() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    lanes = read_json(CONFIG_ROOT / "lanes.json")["lanes"]
    tasks = read_json(CONFIG_ROOT / "tasks.json")["tasks"]
    return lanes, tasks


def default_state() -> dict[str, Any]:
    lanes, tasks = load_config()
    task_states = []
    for task in sorted(tasks, key=lambda item: (item["lane"], item["priority"], item["id"])):
        task_states.append(
            {
                "id": task["id"],
                "lane": task["lane"],
                "priority": task["priority"],
                "title": task["title"],
                "objective": task["objective"],
                "doneCriteria": task["doneCriteria"],
                "status": "queued",
                "claimedAt": None,
                "completedAt": None,
                "note": None,
            }
        )

    lane_states = []
    for lane in sorted(lanes, key=lambda item: (item["priority"], item["id"])):
        lane_states.append(
            {
                "id": lane["id"],
                "title": lane["title"],
                "type": lane["type"],
                "worktree": lane["worktree"],
                "ownership": lane["ownership"],
                "priority": lane["priority"],
                "status": lane["default_status"],
                "activeTaskId": None,
                "lastUpdatedAt": None,
            }
        )

    return {
        "createdAt": now_iso(),
        "updatedAt": now_iso(),
        "lanes": lane_states,
        "tasks": task_states,
    }


def load_state() -> dict[str, Any]:
    if not STATE_PATH.exists():
        state = default_state()
        write_json(STATE_PATH, state)
        return state
    return read_json(STATE_PATH)


def save_state(state: dict[str, Any]) -> None:
    state["updatedAt"] = now_iso()
    write_json(STATE_PATH, state)


def ensure_state(_: argparse.Namespace) -> int:
    with state_lock():
        state = load_state()
        save_state(state)
    print(f"Swarm state ready: {STATE_PATH}")
    return 0


def status(_: argparse.Namespace) -> int:
    state = load_state()
    print(f"Swarm state: {STATE_PATH}")
    print("")
    print("Lanes:")
    for lane in sorted(state["lanes"], key=lambda item: (item["priority"], item["id"])):
        print(
            f"- {lane['id']}: status={lane['status']}, worktree={lane['worktree']}, activeTask={lane['activeTaskId'] or '-'}"
        )

    print("")
    print("Tasks:")
    for task in sorted(state["tasks"], key=lambda item: (item["lane"], item["priority"], item["id"])):
        print(f"- {task['id']}: lane={task['lane']}, status={task['status']}, title={task['title']}")
    return 0


def claim_next(args: argparse.Namespace) -> int:
    with state_lock():
        state = load_state()
        lane = next((item for item in state["lanes"] if item["id"] == args.lane), None)
        if lane is None:
            raise SystemExit(f"Unknown lane '{args.lane}'")

        queued = [
            task
            for task in state["tasks"]
            if task["lane"] == args.lane and task["status"] == "queued"
        ]
        queued.sort(key=lambda item: (item["priority"], item["id"]))
        if not queued:
            print(f"No queued tasks for lane '{args.lane}'")
            return 0

        task = queued[0]
        task["status"] = "in_progress"
        task["claimedAt"] = now_iso()
        lane["activeTaskId"] = task["id"]
        lane["lastUpdatedAt"] = now_iso()
        save_state(state)

    print(f"lane={lane['id']}")
    print(f"task={task['id']}")
    print(f"title={task['title']}")
    print(f"objective={task['objective']}")
    return 0


def complete_task(args: argparse.Namespace) -> int:
    with state_lock():
        state = load_state()
        task = next((item for item in state["tasks"] if item["id"] == args.task_id), None)
        if task is None:
            raise SystemExit(f"Unknown task '{args.task_id}'")

        task["status"] = "completed"
        task["completedAt"] = now_iso()
        task["note"] = args.note

        for lane in state["lanes"]:
            if lane["activeTaskId"] == task["id"]:
                lane["activeTaskId"] = None
                lane["lastUpdatedAt"] = now_iso()

        save_state(state)
    print(f"Completed: {task['id']}")
    return 0


def add_task(args: argparse.Namespace) -> int:
    with state_lock():
        state = load_state()
        if any(item["id"] == args.task_id for item in state["tasks"]):
            raise SystemExit(f"Task '{args.task_id}' already exists")

        state["tasks"].append(
            {
                "id": args.task_id,
                "lane": args.lane,
                "priority": args.priority,
                "title": args.title,
                "objective": args.objective,
                "doneCriteria": args.done_criteria,
                "status": "queued",
                "claimedAt": None,
                "completedAt": None,
                "note": None,
            }
        )
        save_state(state)
    print(f"Added: {args.task_id}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Minimal control plane for multi-session Codex swarm.")
    subparsers = parser.add_subparsers(dest="action", required=True)

    ensure_parser = subparsers.add_parser("ensure-state")
    ensure_parser.set_defaults(func=ensure_state)

    status_parser = subparsers.add_parser("status")
    status_parser.set_defaults(func=status)

    claim_parser = subparsers.add_parser("claim-next")
    claim_parser.add_argument("--lane", required=True)
    claim_parser.set_defaults(func=claim_next)

    complete_parser = subparsers.add_parser("complete")
    complete_parser.add_argument("--task-id", required=True)
    complete_parser.add_argument("--note", default=None)
    complete_parser.set_defaults(func=complete_task)

    add_parser = subparsers.add_parser("add-task")
    add_parser.add_argument("--lane", required=True)
    add_parser.add_argument("--task-id", required=True)
    add_parser.add_argument("--priority", type=int, default=10)
    add_parser.add_argument("--title", required=True)
    add_parser.add_argument("--objective", required=True)
    add_parser.add_argument("--done-criteria", nargs="+", default=[])
    add_parser.set_defaults(func=add_task)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
