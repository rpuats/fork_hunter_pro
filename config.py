# config.py
import os
from pathlib import Path

BASE_DIR = Path(__file__).parent

CONFIG = {
    "min_profit_percent": 1.0,
    "cycle_delay": 15,
    "max_events_per_scraper": 80,
    "log_all_forks": True,
    
    "bookmakers": {
        "winline": {
            "enabled": True,
            "url_live": "https://winline.ru/live",
            "priority": 2
        },
        "olimp": {
            "enabled": True,
            "url_live": "https://www.olimp.bet/live",
            "priority": 1
        },
        "pari": {
            "enabled": True,
            "url_live": "https://www.pari.ru/live/football",
            "priority": 1
        }
    },
    
    "freebet_priorities": {
        "winline": 2,
        "olimp": 3,
        "pari": 1
    }
}
