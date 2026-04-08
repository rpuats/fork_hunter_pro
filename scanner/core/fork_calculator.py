# scanner/core/fork_calculator.py
"""
Fork Calculator - finds arbitrage opportunities across bookmakers.
Supports 1X2, totals, and handicaps.
"""
from typing import List, Dict, Tuple, Optional
from itertools import combinations


class ForkCalculator:
    """Calculates arbitrage opportunities."""
    
    @staticmethod
    def calc_fork_1x2(odds_list: List[Tuple[str, float, float, float]]) -> Optional[Dict]:
        """
        Calculate 1X2 fork from multiple bookmakers.
        odds_list: [(bk_name, odd1, oddX, odd2), ...]
        
        Returns fork info or None if no fork found.
        """
        if len(odds_list) < 2:
            return None
        
        # Find best odds for each outcome across all BKs
        best_1 = max(odds_list, key=lambda x: x[1])
        best_x = max(odds_list, key=lambda x: x[2])
        best_2 = max(odds_list, key=lambda x: x[3])
        
        odd1, bk1 = best_1[1], best_1[0]
        oddX, bkX = best_x[2], best_x[0]
        odd2, bk2 = best_2[3], best_2[0]
        
        # Check if fork exists
        margin = (1/odd1 + 1/oddX + 1/odd2)
        
        if margin < 1:
            profit = (1 - margin) * 100
            
            # Calculate stake distribution for 1000 rub total
            total_stake = 1000
            stake1 = total_stake * (1/odd1) / margin
            stakeX = total_stake * (1/oddX) / margin
            stake2 = total_stake * (1/odd2) / margin
            
            payout = stake1 * odd1  # Same for all outcomes
            
            return {
                'type': '1x2',
                'profit_percent': round(profit, 2),
                'margin': round(margin, 4),
                'total_stake': total_stake,
                'bets': [
                    {'outcome': '1', 'bk': bk1, 'odd': odd1, 'stake': round(stake1, 2)},
                    {'outcome': 'X', 'bk': bkX, 'odd': oddX, 'stake': round(stakeX, 2)},
                    {'outcome': '2', 'bk': bk2, 'odd': odd2, 'stake': round(stake2, 2)},
                ],
                'guaranteed_payout': round(payout, 2),
                'guaranteed_profit': round(payout - total_stake, 2),
                'bks_used': list(set([bk1, bkX, bk2])),
            }
        
        return None
    
    @staticmethod
    def calc_fork_2way(odds_list: List[Tuple[str, float, float]]) -> Optional[Dict]:
        """
        Calculate 2-way fork (totals, handicaps).
        odds_list: [(bk_name, odd_a, odd_b), ...]
        """
        if len(odds_list) < 2:
            return None
        
        best_a = max(odds_list, key=lambda x: x[1])
        best_b = max(odds_list, key=lambda x: x[2])
        
        oddA, bkA = best_a[1], best_a[0]
        oddB, bkB = best_b[2], best_b[0]
        
        margin = (1/oddA + 1/oddB)
        
        if margin < 1:
            profit = (1 - margin) * 100
            total_stake = 1000
            stakeA = total_stake * (1/oddA) / margin
            stakeB = total_stake * (1/oddB) / margin
            payout = stakeA * oddA
            
            return {
                'type': '2way',
                'profit_percent': round(profit, 2),
                'margin': round(margin, 4),
                'total_stake': total_stake,
                'bets': [
                    {'outcome': 'A', 'bk': bkA, 'odd': oddA, 'stake': round(stakeA, 2)},
                    {'outcome': 'B', 'bk': bkB, 'odd': oddB, 'stake': round(stakeB, 2)},
                ],
                'guaranteed_payout': round(payout, 2),
                'guaranteed_profit': round(payout - total_stake, 2),
                'bks_used': list(set([bkA, bkB])),
            }
        
        return None
    
    @staticmethod
    def find_all_forks(
        matched_events: List[Tuple[Dict, Dict, float]],
        min_profit: float = 1.0
    ) -> List[Dict]:
        """
        Find all forks in matched events.
        matched_events: [(event_a, event_b, confidence), ...]
        """
        forks = []
        
        for evt_a, evt_b, confidence in matched_events:
            bk_a = evt_a.get('bookmaker', '')
            bk_b = evt_b.get('bookmaker', '')
            
            if bk_a == bk_b:
                continue
            
            home_a = evt_a.get('home_team', '')
            away_a = evt_a.get('away_team', '')
            
            # 1X2 fork
            odds_1x2 = [
                (bk_a, evt_a.get('home_odds', 0), evt_a.get('draw_odds', 0) or 0, evt_a.get('away_odds', 0)),
                (bk_b, evt_b.get('home_odds', 0), evt_b.get('draw_odds', 0) or 0, evt_b.get('away_odds', 0)),
            ]
            
            # Filter out zero odds
            valid_1x2 = [(bk, o1, ox, o2) for bk, o1, ox, o2 in odds_1x2 if o1 > 1 and o2 > 1]
            
            if len(valid_1x2) >= 2:
                fork = ForkCalculator.calc_fork_1x2(valid_1x2)
                if fork and fork['profit_percent'] >= min_profit:
                    fork['match'] = f"{home_a} vs {away_a}"
                    fork['bookmakers'] = f"{bk_a} vs {bk_b}"
                    fork['confidence'] = round(confidence * 100, 1)
                    fork['is_live'] = evt_a.get('is_live', False)
                    forks.append(fork)
        
        # Sort by profit descending
        forks.sort(key=lambda x: x['profit_percent'], reverse=True)
        return forks
