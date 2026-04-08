# core/surebet_calculator.py
import logging
from typing import List, Dict
from collections import defaultdict

logger = logging.getLogger(__name__)

class SurebetCalculator:
    def __init__(self, min_profit: float = 0.1):  # Понижаем до 0.1%
        self.min_profit = min_profit

    def find_surebets(self, events: List[Dict]) -> List[Dict]:
        surebets = []
        grouped = defaultdict(list)

        for event in events:
            key = event.get("normalized_name") or event.get("name", "")
            if key and len(key) > 5:  # Понижаем порог длины
                grouped[key].append(event)

        for event_name, group in grouped.items():
            if len(group) < 2:
                continue

            for i in range(len(group)):
                for j in range(i + 1, len(group)):
                    e1 = group[i]
                    e2 = group[j]

                    if e1["bookmaker"] == e2["bookmaker"]:
                        continue

                    # 1X2
                    if e1.get("market_type") == "1x2" and e2.get("market_type") == "1x2":
                        profit = self._calc_profit(e1.get("p1"), e2.get("p2"))
                        if profit >= self.min_profit:
                            surebets.append(self._make_surebet(e1, e2, "П1 vs П2", profit))

                    # Тотал 2.5
                    if e1.get("market_type") == "total" and e2.get("market_type") == "total":
                        if abs(e1.get("total_value", 0) - e2.get("total_value", 0)) < 0.1:
                            profit = self._calc_profit(e1.get("over"), e2.get("under"))
                            if profit >= self.min_profit:
                                surebets.append(self._make_surebet(e1, e2, f"ТБ/ТМ {e1.get('total_value')}", profit))

        return sorted(surebets, key=lambda x: x.get("profit_percent", 0), reverse=True)

    def _calc_profit(self, odd1: float, odd2: float) -> float:
        if odd1 <= 1.05 or odd2 <= 1.05:
            return 0.0
        margin = (1 / odd1) + (1 / odd2)
        return round((1 - margin) * 100, 2)

    def _make_surebet(self, e1: Dict, e2: Dict, market_type: str, profit: float) -> Dict:
        odd1 = e1.get("p1") or e1.get("over", 0)
        odd2 = e2.get("p2") or e2.get("under", 0)

        margin = (1 / odd1) + (1 / odd2)
        stake1 = round(100 * (1 / odd1) / margin, 1)
        stake2 = round(100 - stake1, 1)

        return {
            "event_name": e1.get("name", "Unknown"),
            "market_type": market_type,
            "profit_percent": profit,
            "bookmaker1": e1["bookmaker"],
            "bookmaker2": e2["bookmaker"],
            "odd1": round(odd1, 2),
            "odd2": round(odd2, 2),
            "stake1_percent": stake1,
            "stake2_percent": stake2
        }
