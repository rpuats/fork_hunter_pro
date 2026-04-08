# freebet/freebet_handler.py
import logging
from typing import Dict, List

logger = logging.getLogger(__name__)

class FreebetHandler:
    def __init__(self):
        self.freebets = {
            "olimp": {"amount": 1000, "priority": 3, "type": "депозитный"}
        }

    def analyze_opportunity(self, surebet: Dict) -> Dict:
        bk1 = surebet.get("bookmaker1", "").lower()
        bk2 = surebet.get("bookmaker2", "").lower()

        for bk in [bk1, bk2]:
            if bk in self.freebets:
                fb = self.freebets[bk]
                return {
                    "recommended": True,
                    "bk": bk,
                    "amount": fb["amount"],
                    "message": f"Использовать фрибет {fb['amount']} руб на {surebet.get('event_name', '')[:70]} ({bk.upper()})"
                }

        return {"recommended": False, "message": ""}

    def get_recommendations(self, surebets: List[Dict]) -> List[Dict]:
        return [self.analyze_opportunity(sb) for sb in surebets[:8] if self.analyze_opportunity(sb)["recommended"]]
