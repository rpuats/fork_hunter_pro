# core/freebet_hunter.py
"""
Freebet Hunter Module - Поиск вилок специально под отыгрыш бонусов РФ БК.

Алгоритм:
1. Находим вилки где одна из БК = БК с фрибетом
2. Считаем ROI с учетом фрибета: ROI = (прибыль_вилки + фрибет_сумма) / общая_ставка * 100
3. Ранжируем по ROI с фрибетом
4. Показываем обычные вилки и фрибет-вилки отдельно

Бонусы РФ БК (актуальные на 2026):
- Winline: фрибет 5000₽ при регистрации
- Pari: фрибет 2500₽ при регистрации
- Betcity: фрибет 3000₽ при регистрации
- Baltbet: фрибет 2000₽ при регистрации
- Marathon: фрибет 10000₽ при первом депозите
- Zenit: фрибет 1500₽ при регистрации
- Bettery: фрибет 1000₽ при регистрации
"""
from typing import List, Dict, Optional
from dataclasses import dataclass
import logging
import time

logger = logging.getLogger(__name__)


@dataclass
class FreebetOffer:
    bookmaker: str
    amount: float
    min_odds: float = 1.5
    min_turnover: float = 1.0
    description: str = ""


FREEBET_DATABASE: Dict[str, FreebetOffer] = {
    'winline': FreebetOffer(
        bookmaker='winline',
        amount=5000.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 5000₽ при регистрации'
    ),
    'pari': FreebetOffer(
        bookmaker='pari',
        amount=2500.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 2500₽ при регистрации'
    ),
    'betcity': FreebetOffer(
        bookmaker='betcity',
        amount=3000.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 3000₽ при регистрации'
    ),
    'baltbet': FreebetOffer(
        bookmaker='baltbet',
        amount=2000.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 2000₽ при регистрации'
    ),
    'marathon': FreebetOffer(
        bookmaker='marathon',
        amount=10000.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 10000₽ при первом депозите'
    ),
    'zenit': FreebetOffer(
        bookmaker='zenit',
        amount=1500.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 1500₽ при регистрации'
    ),
    'bettery': FreebetOffer(
        bookmaker='bettery',
        amount=1000.0,
        min_odds=1.5,
        min_turnover=1.0,
        description='Фрибет 1000₽ при регистрации'
    ),
}


class FreebetHunter:
    """Поиск вилок специально под отыгрыш бонусов."""

    def __init__(self, min_freebet_roi: float = 5.0):
        self.min_freebet_roi = min_freebet_roi
        self._freebet_offers = FREEBET_DATABASE

    def get_available_freebets(self) -> List[Dict]:
        """Получить список доступных фрибетов."""
        return [
            {
                'bookmaker': offer.bookmaker,
                'amount': offer.amount,
                'min_odds': offer.min_odds,
                'min_turnover': offer.min_turnover,
                'description': offer.description,
            }
            for offer in self._freebet_offers.values()
        ]

    def has_freebet(self, bookmaker: str) -> bool:
        """Проверить есть ли фрибет у букмекера."""
        return bookmaker in self._freebet_offers

    def get_freebet(self, bookmaker: str) -> Optional[FreebetOffer]:
        """Получить информацию о фрибете букмекера."""
        return self._freebet_offers.get(bookmaker)

    def calculate_freebet_roi(
        self,
        surebet: Dict,
        total_stake: float = 10000
    ) -> Optional[Dict]:
        """
        Рассчитать ROI вилки с учетом фрибета.

        Формула:
        ROI_freebet = (прибыль_вилки + фрибет_сумма) / общая_ставка * 100

        Args:
            surebet: словарь вилки из SurebetCalculator
            total_stake: общая сумма ставок

        Returns:
            Словарь с ROI и информацией о фрибете или None
        """
        legs = surebet.get('legs', [])
        if not legs:
            return None

        bookmakers = [leg.get('bookmaker', '') for leg in legs]

        freebet_bk = None
        freebet_amount = 0.0

        for bk in bookmakers:
            if bk in self._freebet_offers:
                offer = self._freebet_offers[bk]
                if offer.amount > freebet_amount:
                    freebet_bk = bk
                    freebet_amount = offer.amount

        if not freebet_bk:
            return None

        profit_percent = surebet.get('profit_percent', 0)
        regular_profit = total_stake * (profit_percent / 100)

        total_profit_with_freebet = regular_profit + freebet_amount
        roi_with_freebet = (total_profit_with_freebet / total_stake) * 100

        return {
            'original_surebet': surebet,
            'freebet_bookmaker': freebet_bk,
            'freebet_amount': freebet_amount,
            'regular_profit': round(regular_profit, 2),
            'total_profit_with_freebet': round(total_profit_with_freebet, 2),
            'regular_roi': round(profit_percent, 2),
            'roi_with_freebet': round(roi_with_freebet, 2),
            'roi_boost': round(roi_with_freebet - profit_percent, 2),
            'is_worthy': roi_with_freebet >= self.min_freebet_roi,
        }

    def find_freebet_surebets(
        self,
        surebets: List[Dict],
        total_stake: float = 10000
    ) -> List[Dict]:
        """
        Найти все вилки с фрибетами и отранжировать по ROI.

        Args:
            surebets: список вилок из SurebetCalculator
            total_stake: общая сумма ставок

        Returns:
            Список вилок с фрибетами, отранжированный по ROI
        """
        freebet_surebets = []

        for surebet in surebets:
            result = self.calculate_freebet_roi(surebet, total_stake)
            if result and result['is_worthy']:
                freebet_surebets.append(result)

        freebet_surebets.sort(
            key=lambda x: x['roi_with_freebet'],
            reverse=True
        )

        logger.info(
            f"FreebetHunter: found {len(freebet_surebets)} "
            f"freebet surebets (min ROI: {self.min_freebet_roi}%)"
        )

        return freebet_surebets

    def get_best_freebet_strategy(
        self,
        surebets: List[Dict],
        total_stake: float = 10000
    ) -> Optional[Dict]:
        """
        Найти лучшую стратегию отыгрыша фрибета.

        Returns:
            Лучшая вилка с фрибетом или None
        """
        freebet_surebets = self.find_freebet_surebets(surebets, total_stake)

        if not freebet_surebets:
            return None

        best = freebet_surebets[0]

        return {
            'strategy': 'freebet_hunt',
            'best_surebet': best,
            'recommendation': (
                f"Ставь на {best['freebet_bookmaker']} с фрибетом {best['freebet_amount']}₽. "
                f"ROI с фрибетом: {best['roi_with_freebet']}% "
                f"(обычный ROI: {best['regular_roi']}%)"
            ),
            'total_profit': best['total_profit_with_freebet'],
            'roi': best['roi_with_freebet'],
        }

    def simulate_freebet_profit(
        self,
        freebet_amount: float,
        min_odds: float = 1.5,
        success_rate: float = 0.95
    ) -> Dict:
        """
        Симулировать прибыль от фрибета.

        Args:
            freebet_amount: сумма фрибета
            min_odds: минимальный коэффициент для ставки
            success_rate: вероятность успешной ставки

        Returns:
            Словарь с результатами симуляции
        """
        expected_return = freebet_amount * min_odds * success_rate
        expected_profit = expected_return - freebet_amount
        roi = (expected_profit / freebet_amount) * 100

        return {
            'freebet_amount': freebet_amount,
            'min_odds': min_odds,
            'success_rate': success_rate,
            'expected_return': round(expected_return, 2),
            'expected_profit': round(expected_profit, 2),
            'roi': round(roi, 2),
        }
