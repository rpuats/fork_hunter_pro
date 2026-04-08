# automation/bookmarklets.py
"""
Browser Bookmarklets - Quick bet placement from any page
"""
from typing import Dict, List


class BookmarkletGenerator:
    """Generates bookmarklet JavaScript for quick betting"""
    
    BOOKMAKER_SELECTORS = {
        'winline': {
            'odds_button': '[data-qa="coefficient-value"]',
            'stake_input': 'input[name="sum"]',
            'bet_button': 'button[data-qa="accept"]',
        },
        'fonbet': {
            'odds_button': '.bet-button__value',
            'stake_input': 'input.bet-input__value',
            'bet_button': 'button[data-id="place-bet"]',
        },
        'betboom': {
            'odds_button': '.koeff__value',
            'stake_input': 'input.stake__input',
            'bet_button': 'button.bet__button',
        },
        'pari': {
            'odds_button': '[class*="outcome"]',
            'stake_input': 'input[name="sum"]',
            'bet_button': 'button[data-testid="place-bet"]',
        },
        'olimp': {
            'odds_button': '.coef',
            'stake_input': 'input.stake',
            'bet_button': 'button.bet-btn',
        },
        '1xstavka': {
            'odds_button': '[data-target="coeff"]',
            'stake_input': 'input[name="sum"]',
            'bet_button': 'button[data-id="place"]',
        },
        'leon': {
            'odds_button': '.kef',
            'stake_input': 'input.stake',
            'bet_button': 'button.bet',
        },
    }
    
    @staticmethod
    def generate_ghost_bet_js(bookmaker: str, selection: str, odds: float, stake: float) -> str:
        """Generate JavaScript bookmarklet for placing a bet"""
        
        selectors = BookmarkletGenerator.BOOKMAKER_SELECTORS.get(bookmaker, {})
        
        js = f"""
        (function() {{
            console.log('Ghost Imperium: Placing bet');
            
            // Find and click odds button
            const oddsBtn = document.querySelector('{selectors.get("odds_button", "")}');
            if (oddsBtn) {{
                oddsBtn.click();
                console.log('Clicked odds button');
            }}
            
            // Wait and fill stake
            setTimeout(() => {{
                const stakeInput = document.querySelector('{selectors.get("stake_input", "")}');
                if (stakeInput) {{
                    stakeInput.value = {stake};
                    stakeInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    console.log('Filled stake: {stake}');
                }}
                
                // Click bet button
                setTimeout(() => {{
                    const betBtn = document.querySelector('{selectors.get("bet_button", "")}');
                    if (betBtn) {{
                        betBtn.click();
                        console.log('Clicked bet button');
                        alert('Ghost Imperium: Bet placed!');
                    }}
                }}, 500);
            }}, 1000);
        }})();
        """
        
        return js
    
    @staticmethod
    def generate_overlay_js() -> str:
        """Generate JavaScript for floating Ghost overlay"""
        
        return """
        (function() {
            // Remove if exists
            const existing = document.getElementById('ghost-imperium-overlay');
            if (existing) existing.remove();
            
            // Create overlay
            const overlay = document.createElement('div');
            overlay.id = 'ghost-imperium-overlay';
            overlay.style.cssText = `
                position: fixed;
                top: 20px;
                right: 20px;
                width: 300px;
                background: linear-gradient(135deg, #1a1a25, #0a0a0f);
                border: 1px solid rgba(0, 212, 255, 0.3);
                border-radius: 12px;
                padding: 16px;
                font-family: 'Inter', sans-serif;
                color: #e0e0e0;
                z-index: 999999;
                box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
            `;
            
            overlay.innerHTML = `
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
                    <div style="font-size: 18px; font-weight: 700; color: #00d4ff;">
                        Ghost Imperium
                    </div>
                    <button onclick="this.parentElement.parentElement.remove()" style="
                        background: none; border: none; color: #888; 
                        font-size: 20px; cursor: pointer;
                    ">x</button>
                </div>
                <div id="ghost-status" style="color: #888; font-size: 12px; margin-bottom: 12px;">
                    Status: Scanning...
                </div>
                <div id="ghost-surebets" style="max-height: 200px; overflow-y: auto;">
                </div>
                <div style="margin-top: 12px; font-size: 11px; color: #666;">
                    Press Ctrl+Shift+G to toggle
                </div>
            `;
            
            document.body.appendChild(overlay);
            
            // Listen for messages from extension
            window.addEventListener('message', (event) => {
                if (event.data.type === 'GHOST_UPDATE') {
                    document.getElementById('ghost-status').textContent = 
                        `Found: ${event.data.surebets} surebets`;
                }
            });
            
            console.log('Ghost Imperium overlay activated');
        })();
        """
    
    @staticmethod
    def generate_keyboard_shortcut_js() -> str:
        """Generate JavaScript for keyboard shortcuts"""
        
        return """
        (function() {
            document.addEventListener('keydown', (e) => {
                // Ctrl+Shift+G = Toggle overlay
                if (e.ctrlKey && e.shiftKey && e.key === 'G') {
                    const overlay = document.getElementById('ghost-imperium-overlay');
                    if (overlay) {
                        overlay.style.display = overlay.style.display === 'none' ? 'block' : 'none';
                    } else {
                        // Load overlay
                        const script = document.createElement('script');
                        script.textContent = `GHOST_OVERLAY_JS`;
                        document.body.appendChild(script);
                    }
                    e.preventDefault();
                }
                
                // Ctrl+Shift+B = Quick bet
                if (e.ctrlKey && e.shiftKey && e.key === 'B') {
                    console.log('Ghost: Quick bet shortcut');
                    // Find first visible odds and place bet
                    const btn = document.querySelector('[data-qa="coefficient-value"], .koeff__value, .kef');
                    if (btn) btn.click();
                }
            });
            
            console.log('Ghost Imperium shortcuts loaded (Ctrl+Shift+G, Ctrl+Shift+B)');
        })();
        """
    
    @staticmethod
    def generate_csv_export(surebets: List[Dict]) -> str:
        """Generate CSV content for surebets"""
        import csv
        import io
        
        output = io.StringIO()
        writer = csv.writer(output)
        
        # Header
        writer.writerow([
            'ID', 'Event', 'Sport', 'Bookmaker1', 'Odds1', 'Stake1',
            'Bookmaker2', 'Odds2', 'Stake2', 'Profit%', 'Profit RUB', 'Live'
        ])
        
        # Data
        for sb in surebets:
            legs = sb.get('legs', [])
            writer.writerow([
                sb.get('id', ''),
                sb.get('event_name', ''),
                sb.get('sport', ''),
                legs[0].get('bookmaker', '') if len(legs) > 0 else '',
                legs[0].get('odds', '') if len(legs) > 0 else '',
                legs[0].get('calculated_stake', '') if len(legs) > 0 else '',
                legs[1].get('bookmaker', '') if len(legs) > 1 else '',
                legs[1].get('odds', '') if len(legs) > 1 else '',
                legs[1].get('calculated_stake', '') if len(legs) > 1 else '',
                sb.get('profit_percent', ''),
                sb.get('estimated_profit', ''),
                sb.get('is_live', '')
            ])
        
        return output.getvalue()
    
    @staticmethod
    def generate_html_page(surebets: List[Dict]) -> str:
        """Generate standalone HTML page with all functionality"""
        
        csv_data = BookmarkletGenerator.generate_csv_export(surebets)
        overlay_js = BookmarkletGenerator.generate_overlay_js()
        shortcut_js = BookmarkletGenerator.generate_keyboard_shortcut_js()
        
        html = f"""
<!DOCTYPE html>
<html lang="ru">
<head>
    <meta charset="UTF-8">
    <title>Ghost Imperium - Bookmarklets</title>
    <style>
        body {{ 
            font-family: 'Inter', sans-serif; 
            background: #0a0a0f; 
            color: #e0e0e0;
            padding: 40px;
        }}
        .card {{
            background: #1a1a25;
            border: 1px solid rgba(0, 212, 255, 0.3);
            border-radius: 12px;
            padding: 20px;
            margin: 20px 0;
        }}
        h1 {{ color: #00d4ff; }}
        h2 {{ color: #00ff88; }}
        code {{
            background: #0a0a0f;
            padding: 4px 8px;
            border-radius: 4px;
            color: #ffaa00;
        }}
        .bookmarklet {{
            display: inline-block;
            background: linear-gradient(135deg, #00d4ff, #00ff88);
            color: #000;
            padding: 12px 24px;
            border-radius: 8px;
            text-decoration: none;
            font-weight: 600;
            cursor: grab;
        }}
        .bookmarklet:hover {{
            transform: translateY(-2px);
        }}
        pre {{
            background: #0a0a0f;
            padding: 16px;
            border-radius: 8px;
            overflow-x: auto;
        }}
        .shortcut {{
            background: #2a2a35;
            padding: 4px 8px;
            border-radius: 4px;
            font-family: monospace;
        }}
    </style>
</head>
<body>
    <h1>Ghost Imperium - Browser Tools</h1>
    
    <div class="card">
        <h2>Bookmarklets</h2>
        <p>Drag these links to your bookmarks bar:</p>
        
        <p>
            <strong>Ghost Overlay:</strong><br>
            <a class="bookmarklet" href="javascript:{overlay_js.replace('"', '&quot;')}">
                Ghost Overlay
            </a>
        </p>
        
        <p>
            <strong>Ghost Shortcuts:</strong><br>
            <a class="bookmarklet" href="javascript:{shortcut_js.replace('"', '&quot;')}">
                Ghost Shortcuts
            </a>
        </p>
    </div>
    
    <div class="card">
        <h2>Keyboard Shortcuts</h2>
        <ul>
            <li><span class="shortcut">Ctrl+Shift+G</span> - Toggle Ghost overlay</li>
            <li><span class="shortcut">Ctrl+Shift+B</span> - Quick bet (click first odds)</li>
        </ul>
    </div>
    
    <div class="card">
        <h2>Export Data</h2>
        <p>Current surebets: <strong>{len(surebets)}</strong></p>
        <a class="bookmarklet" href="data:text/csv;charset=utf-8,{csv_data}">
            Download CSV
        </a>
    </div>
    
    <div class="card">
        <h2>Installation Instructions</h2>
        <ol>
            <li>Drag the bookmarklet links above to your bookmarks bar</li>
            <li>Open your bookmaker's website</li>
            <li>Click the "Ghost Overlay" bookmark</li>
            <li>Use <span class="shortcut">Ctrl+Shift+B</span> for quick betting</li>
        </ol>
    </div>
</body>
</html>
        """
        
        return html


# Save bookmarklet page
def save_bookmarklet_page(filename: str = "ghost_bookmarklets.html"):
    """Save bookmarklet page to file"""
    html = BookmarkletGenerator.generate_html_page([])
    
    with open(filename, 'w', encoding='utf-8') as f:
        f.write(html)
    
    print(f"Saved bookmarklets to {filename}")
