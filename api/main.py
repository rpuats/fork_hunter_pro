# api/main.py
from __future__ import annotations
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import RedirectResponse
from contextlib import asynccontextmanager
import structlog
import os
from typing import Optional

from api.routes import router
from api.websocket import router as ws_router, ws_manager
from scanner.engine import GhostScanner, ScannerConfig
from services.database import Database

structlog.configure(
    processors=[
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer()
    ]
)
logger = structlog.get_logger()

scanner: Optional[GhostScanner] = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    global scanner
    
    database = Database()
    await database.init()
    
    config = ScannerConfig(
        min_profit=0.5,
        cycle_interval=3.0,
        max_events_per_source=200,
        cache_ttl=10.0
    )
    
    scanner = GhostScanner(database, config)
    await scanner.start()
    
    from api.routes import set_scanner
    set_scanner(scanner)
    
    logger.info("Ghost Imperium started successfully")
    
    yield
    
    if scanner:
        await scanner.stop()


app = FastAPI(
    title="👻 Ghost Imperium API",
    description="Professional sports arbitrage scanner - Best fork finder",
    version="2.0.0",
    lifespan=lifespan
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

app.include_router(router)
app.include_router(ws_router)


@app.get("/")
async def root():
    return RedirectResponse(url="/web/index.html")


@app.get("/health")
async def health():
    global scanner
    return {
        "status": "healthy",
        "scanner": scanner.is_running if scanner else False,
        "version": "2.0.0"
    }


@app.get("/web/{path}")
async def serve_web(path: str):
    web_path = os.path.join(os.path.dirname(__file__), "..", "web", path)
    if os.path.exists(web_path):
        from fastapi.responses import FileResponse
        return FileResponse(web_path)
    return {"error": "Not found"}


@app.on_event("startup")
async def startup_event():
    logger.info("🚀 Ghost Imperium API starting...")


@app.on_event("shutdown")
async def shutdown_event():
    logger.info("👋 Ghost Imperium API shutting down...")
