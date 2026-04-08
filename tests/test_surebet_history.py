# tests/test_surebet_history.py
import pytest
import sys
import os
import asyncio
import json
from datetime import datetime, date

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from core.surebet_history import SurebetHistory


DB_PATH = ":memory:"


def make_surebet(**overrides):
    base = {
        "id": "test001",
        "event_name": "Team A vs Team B",
        "sport": "football",
        "market_type": "2-way",
        "is_live": True,
        "profit_percent": 2.5,
        "total_stake": 10000,
        "estimated_profit": 250,
        "legs": [
            {
                "bookmaker": "winline",
                "market": "1",
                "selection": "П1",
                "odds": 2.10,
                "event_name": "Team A vs Team B",
                "calculated_stake": 5122,
                "stake_percent": 51.22,
            },
            {
                "bookmaker": "pari",
                "market": "2",
                "selection": "П2",
                "odds": 2.05,
                "event_name": "Team A vs Team B",
                "calculated_stake": 4878,
                "stake_percent": 48.78,
            },
        ],
        "bookmakers": ["winline", "pari"],
        "found_at": "2026-04-01T12:00:00",
    }
    base.update(overrides)
    return base


@pytest.fixture
async def history():
    h = SurebetHistory(db_path=DB_PATH)
    await h.init()
    yield h
    await h.close()


@pytest.mark.asyncio
class TestSaveSurebet:
    async def test_save_returns_id(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        sb = make_surebet()
        sb_id = await h.save_surebet(sb)
        assert sb_id == "test001"
        await h.close()

    async def test_save_generates_id_if_missing(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        sb = make_surebet()
        del sb["id"]
        sb_id = await h.save_surebet(sb)
        assert sb_id is not None and len(sb_id) > 0
        await h.close()


@pytest.mark.asyncio
class TestGetAllSurebets:
    async def test_empty_db_returns_empty(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        result = await h.get_all_surebets()
        assert result == []
        await h.close()

    async def test_returns_saved_surebet(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet())
        result = await h.get_all_surebets()
        assert len(result) == 1
        assert result[0]["event_name"] == "Team A vs Team B"
        assert result[0]["bookmakers"] == ["winline", "pari"]
        await h.close()

    async def test_limit_works(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        for i in range(15):
            await h.save_surebet(make_surebet(id=f"sb_{i}", event_name=f"Match {i}"))
        result = await h.get_all_surebets(limit=5)
        assert len(result) == 5
        await h.close()

    async def test_legs_are_dicts(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet())
        result = await h.get_all_surebets()
        assert isinstance(result[0]["legs"], list)
        assert isinstance(result[0]["legs"][0], dict)
        assert result[0]["legs"][0]["odds"] == 2.10
        await h.close()


@pytest.mark.asyncio
class TestGetByDate:
    async def test_returns_surebet_for_date(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet(found_at="2026-04-01T12:00:00"))
        result = await h.get_surebets_by_date(date(2026, 4, 1))
        assert len(result) == 1

    async def test_returns_empty_for_other_date(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet(found_at="2026-04-01T12:00:00"))
        result = await h.get_surebets_by_date(date(2026, 5, 2))
        assert len(result) == 0
        await h.close()


@pytest.mark.asyncio
class TestGetStats:
    async def test_empty_stats(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        stats = await h.get_surebet_stats()
        assert stats["total"] == 0
        assert stats["avg_profit"] == 0
        await h.close()

    async def test_stats_with_data(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet(id="s1", profit_percent=3.0, bookmakers=["winline", "pari"]))
        await h.save_surebet(make_surebet(id="s2", profit_percent=1.5, bookmakers=["betcity", "marathon"]))
        stats = await h.get_surebet_stats()
        assert stats["total"] == 2
        assert stats["avg_profit"] == 2.25
        assert "winline" in stats["top_bookmakers"]
        await h.close()

    async def test_top_bookmakers_ranking(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        for i in range(5):
            await h.save_surebet(make_surebet(id=f"s{i}", bookmakers=["winline", "pari"]))
        await h.save_surebet(make_surebet(id="s6", bookmakers=["betcity", "marathon"]))
        stats = await h.get_surebet_stats()
        top = stats["top_bookmakers"]
        assert top["winline"] == 5
        assert top["pari"] == 5
        assert top["betcity"] == 1
        await h.close()


@pytest.mark.asyncio
class TestBookmakerHeatmap:
    async def test_heatmap_empty(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        hm = await h.get_bookmaker_heatmap()
        assert hm == {}
        await h.close()

    async def test_heatmap_counts(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet(id="h1", bookmakers=["winline", "pari"]))
        await h.save_surebet(make_surebet(id="h2", bookmakers=["winline", "betcity"]))
        hm = await h.get_bookmaker_heatmap()
        assert hm["winline"] == 2
        assert hm["pari"] == 1
        assert hm["betcity"] == 1
        await h.close()


@pytest.mark.asyncio
class TestOverwrite:
    async def test_replace_on_duplicate_id(self):
        h = SurebetHistory(db_path=DB_PATH)
        await h.init()
        await h.save_surebet(make_surebet(id="dup1", profit_percent=1.0))
        await h.save_surebet(make_surebet(id="dup1", profit_percent=5.0))
        result = await h.get_all_surebets()
        assert len(result) == 1
        assert result[0]["profit_percent"] == 5.0
        await h.close()
