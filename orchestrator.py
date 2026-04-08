import asyncio
import json
import logging
import os
from datetime import datetime
from typing import List, Dict, Any

logger = logging.getLogger(__name__)

# Lightweight task queue persisted to disk
TODO_FILE = os.path.join(os.path.dirname(__file__), "todo.json")

def load_todos() -> List[Dict[str, Any]]:
    if not os.path.exists(TODO_FILE):
        return []
    try:
        with open(TODO_FILE, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception:
        return []

def save_todos(todos: List[Dict[str, Any]]):
    with open(TODO_FILE, 'w', encoding='utf-8') as f:
        json.dump(todos, f, ensure_ascii=False, indent=2)

def add_todo(content: str, priority: str = 'medium', status: str = 'pending'):
    todos = load_todos()
    todos.append({
        'id': content[:8] + '-' + str(len(todos) + 1),
        'content': content,
        'status': status,
        'priority': priority,
        'created_at': datetime.utcnow().isoformat()
    })
    save_todos(todos)

def mark_todo_completed(idx: int):
    todos = load_todos()
    if 0 <= idx < len(todos):
        todos[idx]['status'] = 'completed'
        save_todos(todos)


async def _run_single_parser(parser):
    slug = getattr(parser, 'slug', 'unknown')
    name = getattr(parser, 'name', slug)
    try:
        events = await parser.get_events()
        # Simple report
        return {'slug': slug, 'name': name, 'count': len(events), 'ok': True}
    except Exception as e:
        logger.error(f"Parser {name} ({slug}) failed: {e}")
        return {'slug': slug, 'name': name, 'count': 0, 'ok': False, 'error': str(e)}

def discover_parsers_by_slug(slugs: List[str]):
    # Lazy import of ALL_PARSERS to avoid hard dependencies at import time
    try:
        from scanner.parsers import ALL_PARSERS  # type: ignore
        parsers = []
        for cls in ALL_PARSERS:
            slug = getattr(cls, 'slug', '')
            if slug in slugs:
                try:
                    parsers.append(cls())
                except Exception:
                    pass
        return parsers
    except Exception as e:
        logger.error(f"Failed to discover parsers: {e}")
        return []

async def run_cycle(enabled_slugs: List[str] = None, per_parser_timeout: int = 60):
    if enabled_slugs is None:
        enabled_slugs = ['winline', 'pari', 'betcity', 'marathon', 'zenit', 'baltbet', 'bettery']

    parsers = discover_parsers_by_slug([s for s in enabled_slugs if s is not None])
    if not parsers:
        logger.info("No parsers available for orchestrator cycle.")
        return

    tasks = [asyncio.create_task(_run_single_parser(p)) for p in parsers]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    # Aggregate results and push follow-up tasks
    total = 0
    for r in results:
        if isinstance(r, dict):
            total += r.get('count', 0)
            if not r.get('ok', True):
                add_todo(f"Retry parser {r.get('name','unknown')} ({r.get('slug','')})", priority='high')
        else:
            add_todo("Unknown parser result in orchestrator cycle", priority='low')

    logger.info(f"Orchestrator cycle done. Total events found: {total}")

    # Simple follow-up: propose to test new parsers if available in config
    add_todo("Test Tennisi/Bet-M/Melbet with VPN/proxy for blocked sources", priority='medium')

def main_loop():
    import asyncio
    asyncio.run(run_cycle())

if __name__ == '__main__':
    main_loop()
