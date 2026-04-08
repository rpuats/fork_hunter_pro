# api/websocket.py
import asyncio
from fastapi import APIRouter, WebSocket, WebSocketDisconnect
from typing import Set, List
import logging
import json

router = APIRouter()
logger = logging.getLogger(__name__)


class WSConnectionManager:
    def __init__(self):
        self.active_connections: Set[WebSocket] = set()
        self._lock = asyncio.Lock()
    
    async def connect(self, websocket: WebSocket):
        await websocket.accept()
        async with self._lock:
            self.active_connections.add(websocket)
        logger.info(f"WebSocket connected. Total: {len(self.active_connections)}")
    
    async def disconnect(self, websocket: WebSocket):
        async with self._lock:
            self.active_connections.discard(websocket)
        logger.info(f"WebSocket disconnected. Total: {len(self.active_connections)}")
    
    async def broadcast(self, message: dict):
        disconnected = set()
        
        async with self._lock:
            connections = list(self.active_connections)
        
        for connection in connections:
            try:
                await connection.send_json(message)
            except Exception:
                disconnected.add(connection)
        
        async with self._lock:
            self.active_connections -= disconnected
    
    async def send_surebets_update(self, surebets: List[dict]):
        await self.broadcast({
            "type": "surebets",
            "data": surebets,
            "count": len(surebets)
        })
    
    async def send_new_surebet(self, surebet: dict):
        await self.broadcast({
            "type": "new_surebet",
            "data": surebet
        })
    
    async def send_stats_update(self, stats: dict):
        await self.broadcast({
            "type": "stats",
            "data": stats
        })
    
    async def send_alert(self, message: str, level: str = "info"):
        await self.broadcast({
            "type": "alert",
            "message": message,
            "level": level
        })
    
    def get_connection_count(self) -> int:
        return len(self.active_connections)


ws_manager = WSConnectionManager()


@router.websocket("/ws/v1/surebets")
async def websocket_surebets(websocket: WebSocket):
    await ws_manager.connect(websocket)
    
    try:
        await websocket.send_json({
            "type": "connected",
            "message": "Connected to Ghost Imperium"
        })
        
        while True:
            data = await websocket.receive_text()
            
            if data == "ping":
                await websocket.send_text("pong")
            
            elif data.startswith("subscribe:"):
                channel = data.split(":", 1)[1]
                await websocket.send_json({
                    "type": "subscribed",
                    "channel": channel
                })
            
            elif data == "get_stats":
                from api.routes import scanner
                if scanner:
                    await websocket.send_json({
                        "type": "stats",
                        "data": scanner.get_stats()
                    })
            
            elif data == "get_surebets":
                from api.routes import scanner
                if scanner:
                    await websocket.send_json({
                        "type": "surebets",
                        "data": scanner.get_top_surebets(20)
                    })
    
    except WebSocketDisconnect:
        pass
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
    finally:
        await ws_manager.disconnect(websocket)


async def broadcast_new_surebets(surebets: List[dict]):
    await ws_manager.broadcast({
        "type": "new_surebets",
        "count": len(surebets),
        "data": surebets
    })


async def broadcast_stats(stats: dict):
    await ws_manager.broadcast({
        "type": "stats_update",
        "data": stats
    })
