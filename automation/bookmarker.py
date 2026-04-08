# automation/bookmarker.py
from typing import Dict, List
import json


class BookmarkletGenerator:
    """
    Generates bookmarklets for quick betting
    """
    
    BOOKMAKER_TEMPLATES = {
        'winline': {
            'name': 'Winline',
            'url': 'https://winline.ru',
            'search': "document.querySelector('.search-input')?.value = '{event}'",
            'bet': "javascript:(function(){{var odds=document.querySelectorAll('.coeff');if(odds[0]){{odds[0].click();setTimeout(function(){{document.querySelector('.bet-sum input').value={stake};}},500);}}}})()"
        },
        'olimp': {
            'name': 'Olimp',
            'url': 'https://www.olimp.bet',
            'search': "document.querySelector('.search-line')?.value = '{event}'",
            'bet': "javascript:(function(){{var b=document.querySelectorAll('.bets-item');if(b[0]){{b[0].click();setTimeout(function(){{document.querySelector('.sum-input input').value={stake};}},500);}}}})()"
        },
        'pari': {
            'name': 'Pari',
            'url': 'https://www.pari.ru',
            'search': "document.querySelector('.live-search input')?.value = '{event}'",
            'bet': "javascript:(function(){{var o=document.querySelectorAll('.outcome-value');if(o[0]){{o[0].click();setTimeout(function(){{document.querySelector('.bet-amount input').value={stake};}},500);}}}})()"
        },
        'fonbet': {
            'name': 'Fonbet',
            'url': 'https://www.fonbet.ru',
            'search': "document.querySelector('.search input')?.value = '{event}'",
            'bet': "javascript:(function(){{var c=document.querySelectorAll('.client-odds');if(c[0]){{c[0].click();setTimeout(function(){{document.querySelector('.bet-amount input').value={stake};}},500);}}}})()"
        },
    }
    
    @classmethod
    def generate_bookmarklet(
        cls,
        bookmaker: str,
        event_name: str,
        selection: str,
        odds: float,
        stake: float
    ) -> str:
        template = cls.BOOKMAKER_TEMPLATES.get(bookmaker)
        if not template:
            return None
        
        search_script = template['search'].format(event=event_name)
        bet_script = template['bet'].format(stake=stake)
        
        return f"{search_script};{bet_script}"
    
    @classmethod
    def generate_all_bookmarklets(
        cls,
        surebet: Dict,
        total_stake: float = 10000
    ) -> List[Dict]:
        bookmarklets = []
        
        for leg in surebet.get('legs', []):
            bookmarklet = cls.generate_bookmarklet(
                bookmaker=leg['bookmaker'],
                event_name=leg.get('event_name', surebet.get('event_name', '')),
                selection=leg['selection'],
                odds=leg['odds'],
                stake=leg.get('calculated_stake', total_stake / len(surebet.get('legs', [1])))
            )
            
            if bookmarklet:
                bookmarklets.append({
                    'bookmaker': leg['bookmaker'],
                    'selection': leg['selection'],
                    'odds': leg['odds'],
                    'stake': leg.get('calculated_stake'),
                    'bookmarklet': bookmarklet
                })
        
        return bookmarklets
    
    @classmethod
    def generate_html_page(cls, surebets: List[Dict]) -> str:
        html = '''<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Ghost Imperium - Bookmarklets</title>
    <style>
        body { font-family: Arial, sans-serif; background: #1a1a2e; color: #fff; padding: 20px; }
        .bookmarklet-container { background: #16213e; padding: 20px; margin: 10px 0; border-radius: 8px; }
        .bookmaker-name { font-size: 20px; font-weight: bold; margin-bottom: 10px; }
        .bookmarklet-link { 
            display: inline-block; 
            padding: 10px 20px; 
            background: #00ff88; 
            color: #000; 
            text-decoration: none; 
            border-radius: 5px;
            font-weight: bold;
            cursor: grab;
        }
        .bookmarklet-link:hover { background: #00cc6a; }
        .odds { color: #00d4ff; }
        .stake { color: #ffaa00; }
        .event-name { color: #888; margin: 10px 0; }
        .instructions { margin-top: 30px; padding: 20px; background: #0a0a0f; border-radius: 8px; }
        h1 { color: #00d4ff; }
        code { background: #0a0a0f; padding: 2px 6px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1>👻 Ghost Imperium - Quick Betting</h1>
    <p>Drag the buttons below to your bookmarks bar</p>
'''
        
        for i, sb in enumerate(surebets[:5]):
            bookmarklets = cls.generate_all_bookmarklets(sb)
            profit = sb.get('profit_percent', 0)
            
            html += f'''
    <div class="bookmarklet-container">
        <div class="event-name">{sb.get('event_name', 'Unknown')}</div>
        <div style="color: #00ff88;">Profit: +{profit:.2f}%</div>
'''
            
            for bm in bookmarklets:
                short_code = bm['bookmarklet'][:100] + '...' if len(bm['bookmarklet']) > 100 else bm['bookmarklet']
                html += f'''
        <p>
            <strong>{bm['bookmaker']}</strong>: 
            <span class="odds">K{bm['odds']:.2f}</span> - 
            <span class="stake">{bm['stake']:.0f}₽</span>
        </p>
        <a class="bookmarklet-link" href="{bm['bookmarklet']}">
            Bet {bm['bookmaker']} - {bm['selection']}
        </a>
'''
            
            html += '</div>'
        
        html += '''
    <div class="instructions">
        <h2>📖 How to use</h2>
        <ol>
            <li>Open the bookmaker website</li>
            <li>Drag the bookmarklet buttons to your bookmarks bar</li>
            <li>Click the bookmarklet when on the event page</li>
            <li>The stake will be automatically entered</li>
        </ol>
    </div>
</body>
</html>
'''
        return html
