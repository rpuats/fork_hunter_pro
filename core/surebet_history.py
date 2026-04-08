# core/surebet_history.py
import aiosqlite
import json
import uuid
from datetime import datetime, date
from typing import List, Dict, Optional
from collections import defaultdict
import logging

logger = logging.getLogger(__name__)


class SurebetHistory:
    """Persistent surebet history with analytics and stats."""

    def __init__(self, db_path: str = "ghost_imperium.db"):
        self.db_path = db_path
        self.db: Optional[aiosqlite.Connection] = None

    async def init(self):
        self.db = await aiosqlite.connect(self.db_path)
        await self.db.execute("PRAGMA journal_mode=WAL")
        await self.db.execute("""
            CREATE TABLE IF NOT EXISTS surebet_history (
                id TEXT PRIMARY KEY,
                event_name TEXT,
                sport TEXT,
                market_type TEXT,
                is_live INTEGER,
                profit_percent REAL,
                total_stake REAL,
                estimated_profit REAL,
                bookmakers TEXT,
                legs TEXT,
                found_at TEXT,
                expires_at TEXT
            )
        """)
        await self.db.commit()

    async def close(self):
        if self.db:
            await self.db.close()
            self.db = None

    async def save_surebet(self, surebet: Dict) -> str:
        assert self.db is not None
        sb_id = surebet.get("id") or str(uuid.uuid4())[:8]
        found_at = surebet.get("found_at", datetime.utcnow().isoformat())
        if isinstance(found_at, datetime):
            found_at = found_at.isoformat()
        expires_at = surebet.get("expires_at")
        if isinstance(expires_at, datetime):
            expires_at = expires_at.isoformat()
        legs = surebet.get("legs", [])
        if legs and hasattr(legs[0], "__dict__"):
            legs = [leg.__dict__ if hasattr(leg, "__dict__") else leg for leg in legs]
        bookmakers = surebet.get("bookmakers", [])
        if not bookmakers and legs:
            bookmakers = [leg.get("bookmaker", "") for leg in legs if leg.get("bookmaker")]
        await self.db.execute(
            """
            INSERT OR REPLACE INTO surebet_history
            (id, event_name, sport, market_type, is_live, profit_percent,
             total_stake, estimated_profit, bookmakers, legs, found_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                sb_id,
                surebet.get("event_name", ""),
                surebet.get("sport", ""),
                surebet.get("market_type", ""),
                int(surebet.get("is_live", False)),
                surebet.get("profit_percent", 0.0),
                surebet.get("total_stake", 0.0),
                surebet.get("estimated_profit", 0.0),
                json.dumps(bookmakers),
                json.dumps(legs),
                found_at,
                expires_at,
            ),
        )
        await self.db.commit()
        return sb_id

    async def get_all_surebets(self, limit: int = 1000) -> List[Dict]:
        assert self.db is not None
        cursor = await self.db.execute(
            "SELECT * FROM surebet_history ORDER BY found_at DESC LIMIT ?",
            (limit,),
        )
        rows = await cursor.fetchall()
        columns = [d[0] for d in cursor.description]
        result = []
        for row in rows:
            record = dict(zip(columns, row))
            record["bookmakers"] = json.loads(record["bookmakers"] or "[]")
            record["legs"] = json.loads(record["legs"] or "[]")
            record["is_live"] = bool(record["is_live"])
            result.append(record)
        return result

    async def get_surebets_by_date(self, target_date: date) -> List[Dict]:
        assert self.db is not None
        day_str = target_date.isoformat()
        cursor = await self.db.execute(
            "SELECT * FROM surebet_history WHERE found_at LIKE ? ORDER BY found_at DESC",
            (f"{day_str}%",),
        )
        rows = await cursor.fetchall()
        columns = [d[0] for d in cursor.description]
        result = []
        for row in rows:
            record = dict(zip(columns, row))
            record["bookmakers"] = json.loads(record["bookmakers"] or "[]")
            record["legs"] = json.loads(record["legs"] or "[]")
            record["is_live"] = bool(record["is_live"])
            result.append(record)
        return result

    async def get_surebet_stats(self) -> Dict:
        assert self.db is not None
        cursor = await self.db.execute("SELECT COUNT(*), AVG(profit_percent), AVG(estimated_profit) FROM surebet_history")
        total, avg_profit, avg_est = await cursor.fetchone()

        avg_lifespan = 0.0
        cursor2 = await self.db.execute(
            "SELECT AVG(JULIANDAY(expires_at) - JULIANDAY(found_at)) FROM surebet_history WHERE expires_at IS NOT NULL"
        )
        lifespan_row = await cursor2.fetchone()
        if lifespan_row and lifespan_row[0] is not None:
            avg_lifespan = lifespan_row[0] * 86400  # days → seconds

        cursor3 = await self.db.execute(
            "SELECT bookmakers FROM surebet_history"
        )
        bk_counter: Dict[str, int] = defaultdict(int)
        async for row in cursor3:
            for bk in json.loads(row[0] or "[]"):
                bk_counter[bk] += 1
        top_bookmakers = sorted(bk_counter.items(), key=lambda x: x[1], reverse=True)[:10]

        return {
            "total": total or 0,
            "avg_profit": round(avg_profit or 0, 2),
            "avg_lifespan_seconds": round(avg_lifespan, 2),
            "top_bookmakers": dict(top_bookmakers),
        }

    async def get_bookmaker_heatmap(self) -> Dict[str, int]:
        assert self.db is not None
        cursor = await self.db.execute("SELECT bookmakers FROM surebet_history")
        counter: Dict[str, int] = defaultdict(int)
        async for row in cursor:
            for bk in json.loads(row[0] or "[]"):
                counter[bk] += 1
        return dict(sorted(counter.items(), key=lambda x: x[1], reverse=True))
