# services/database.py
import aiosqlite
from datetime import datetime
from typing import List, Optional, Dict
import json


class Database:
    def __init__(self, path: str = "ghost_imperium.db"):
        self.path = path
        self.db: Optional[aiosqlite.Connection] = None
    
    async def init(self):
        self.db = await aiosqlite.connect(self.path)
        assert self.db is not None
        await self.db.execute("""
            CREATE TABLE IF NOT EXISTS surebets (
                id TEXT PRIMARY KEY,
                event_name TEXT,
                sport TEXT,
                profit_percent REAL,
                total_stake REAL,
                estimated_profit REAL,
                bookmakers TEXT,
                market_type TEXT,
                is_live INTEGER,
                data TEXT,
                found_at TEXT
            )
        """)
        await self.db.execute("""
            CREATE TABLE IF NOT EXISTS stakes (
                id TEXT PRIMARY KEY,
                surebet_id TEXT,
                bookmaker TEXT,
                event_name TEXT,
                selection TEXT,
                odds REAL,
                stake_amount REAL,
                status TEXT,
                placed_at TEXT
            )
        """)
        await self.db.commit()
    
    async def save_surebet(self, surebet: Dict):
        assert self.db is not None
        await self.db.execute("""
            INSERT OR REPLACE INTO surebets 
            (id, event_name, sport, profit_percent, total_stake, estimated_profit, bookmakers, market_type, is_live, data, found_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            surebet['id'],
            surebet['event_name'],
            surebet['sport'],
            surebet['profit_percent'],
            surebet['total_stake'],
            surebet['estimated_profit'],
            json.dumps(surebet['bookmakers']),
            surebet['market_type'],
            int(surebet['is_live']),
            json.dumps(surebet),
            datetime.utcnow().isoformat()
        ))
        await self.db.commit()
    
    async def get_recent_surebets(self, limit: int = 50) -> List[Dict]:
        assert self.db is not None
        cursor = await self.db.execute("""
            SELECT data FROM surebets ORDER BY found_at DESC LIMIT ?
        """, (limit,))
        rows = await cursor.fetchall()
        return [json.loads(row[0]) for row in rows]
    
    async def save_stake(self, stake: Dict):
        assert self.db is not None
        await self.db.execute("""
            INSERT INTO stakes 
            (id, surebet_id, bookmaker, event_name, selection, odds, stake_amount, status, placed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            stake['id'],
            stake['surebet_id'],
            stake['bookmaker'],
            stake['event_name'],
            stake['selection'],
            stake['odds'],
            stake['stake_amount'],
            stake['status'],
            datetime.utcnow().isoformat()
        ))
        await self.db.commit()
    
    async def close(self):
        if self.db:
            await self.db.close()
