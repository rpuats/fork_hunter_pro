#!/usr/bin/env python3
"""
AI Parser Generator for Bookmaker Websites

Generates Rust parsers from bookmaker URLs using LLM analysis.
Supports OpenAI GPT-4, Claude, Gemini, and local Ollama models.

Usage:
    python generate_parser.py --url "https://bk.ru/football" --name "NewBk" --model gpt-4o
    python generate_parser.py --url "https://bk.ru/football" --name "NewBk" --model claude
    python generate_parser.py --url "https://bk.ru/football" --name "NewBk" --model gemini
    python generate_parser.py --url "https://bk.ru/football" --name "NewBk" --model ollama
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path
from datetime import datetime

# Playwright imports
try:
    from playwright.sync_api import sync_playwright
    HAS_PLAYWRIGHT = True
except ImportError:
    HAS_PLAYWRIGHT = False
    print("⚠️  Playwright not installed. Install with: pip install playwright")
    print("    Running in dry-run mode (no HTML extraction)")

# LLM imports
try:
    from openai import OpenAI
    HAS_OPENAI = True
except ImportError:
    HAS_OPENAI = False

# ============================================================
# HTML Extraction via Playwright
# ============================================================

def extract_html(url: str, wait: int = 5, scroll: bool = True) -> str:
    """Load URL with Playwright and extract HTML structure."""
    if not HAS_PLAYWRIGHT:
        return "<html><body>Playwright not available</body></html>"

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        context = browser.new_context(
            viewport={"width": 1920, "height": 1080},
            user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        )
        page = context.new_page()

        # Block images/analytics for speed
        page.route("**/*.{png,jpg,jpeg,gif,svg,webp}", lambda route: route.abort())
        page.route("**/analytics/**", lambda route: route.abort())
        page.route("**/ads/**", lambda route: route.abort())

        print(f"🌐 Loading {url}...")
        page.goto(url, wait_until="domcontentloaded", timeout=30000)

        if scroll:
            print("📜 Scrolling to load lazy content...")
            page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
            time.sleep(1)
            page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
            time.sleep(1)

        # Wait additional time for JS rendering
        print(f"⏳ Waiting {Wait}s for JS rendering...")
        time.sleep(wait)

        # Get simplified HTML (structure only, not content)
        html = page.evaluate("""() => {
            // Get structure: all elements with class/id/data attributes
            const elements = document.querySelectorAll('*[class], *[id], *[data-*]');
            const result = [];
            elements.forEach((el, i) => {
                if (i > 2000) return; // Limit to avoid huge payloads
                const tag = el.tagName.toLowerCase();
                const cls = el.className || '';
                const id = el.id || '';
                const text = (el.textContent || '').trim().substring(0, 50);
                if (tag && (cls || id || text)) {
                    result.push({ tag, class: cls, id, text });
                }
            });
            return JSON.stringify(result);
        }""")

        browser.close()
        return html

# ============================================================
# LLM Analysis
# ============================================================

ANALYSIS_PROMPT = """You are an expert web scraping assistant specializing in bookmaker websites.

Analyze the following HTML structure from a bookmaker website and extract CSS selectors for sports betting data.

BOOKMAKER INFO:
- Name: {name}
- URL: {url}

TASK:
Find CSS selectors for extracting:
1. Match events (home team, away team, league/tournament name)
2. Odds coefficients (1X2: home win, draw, away win)
3. Total markets (Over/Under)
4. Handicap markets
5. Is the event live or pre-match?

HTML STRUCTURE:
{html}

RESPONSE FORMAT (JSON only, no extra text):
```json
{{
  "event_container": "CSS selector for each match container",
  "home_team": "CSS selector for home team name",
  "away_team": "CSS selector for away team name",
  "league": "CSS selector for tournament/league name",
  "odds_container": "CSS selector for odds container",
  "odds_1x2": "CSS selector for 1X2 odds buttons",
  "odds_total_over": "CSS selector for Total Over odds",
  "odds_total_under": "CSS selector for Total Under odds",
  "odds_handicap": "CSS selector for Handicap odds",
  "live_indicator": "CSS selector for live badge (if any)",
  "confidence": 0.0-1.0,
  "notes": "Any special instructions for parsing this site"
}}
```

IMPORTANT:
- Only return valid JSON
- Use specific selectors (e.g., .event-card__team--home, not just .team)
- If a market is not found, use null
- Confidence should reflect how sure you are about the selectors
"""


def analyze_with_gpt4(html: str, name: str, url: str, api_key: str) -> dict:
    """Analyze HTML using GPT-4."""
    if not HAS_OPENAI:
        raise ImportError("Install openai: pip install openai")

    client = OpenAI(api_key=api_key)

    # Truncate HTML to avoid token limits
    max_tokens = 60000
    if len(html) > max_tokens:
        html = html[:max_tokens]

    response = client.chat.completions.create(
        model="gpt-4o",
        messages=[
            {"role": "system", "content": "You are an expert web scraping assistant specializing in bookmaker websites."},
            {"role": "user", "content": ANALYSIS_PROMPT.format(name=name, url=url, html=html)}
        ],
        temperature=0.1,
        max_tokens=2000,
        response_format={"type": "json_object"}
    )

    result = response.choices[0].message.content
    return json.loads(result)


def analyze_with_gemini(html: str, name: str, url: str, api_key: str) -> dict:
    """Analyze HTML using Gemini API."""
    import requests

    # Truncate HTML
    max_tokens = 60000
    if len(html) > max_tokens:
        html = html[:max_tokens]

    prompt = ANALYSIS_PROMPT.format(name=name, url=url, html=html)

    response = requests.post(
        f"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}",
        json={
            "contents": [{
                "parts": [{"text": f"Return only valid JSON:\n{prompt}"}]
            }],
            "generationConfig": {
                "temperature": 0.1,
                "maxOutputTokens": 2000,
                "responseMimeType": "application/json"
            }
        }
    )

    result = response.json()["candidates"][0]["content"]["parts"][0]["text"]
    # Extract JSON from markdown code blocks
    if "```json" in result:
        result = result.split("```json")[1].split("```")[0].strip()
    elif "```" in result:
        result = result.split("```")[1].split("```")[0].strip()

    return json.loads(result)


def analyze_with_claude(html: str, name: str, url: str, api_key: str) -> dict:
    """Analyze HTML using Claude API."""
    import anthropic

    client = anthropic.Anthropic(api_key=api_key)

    # Truncate HTML
    max_tokens = 60000
    if len(html) > max_tokens:
        html = html[:max_tokens]

    prompt = ANALYSIS_PROMPT.format(name=name, url=url, html=html)

    response = client.messages.create(
        model="claude-3-5-sonnet-20241022",
        max_tokens=2000,
        temperature=0.1,
        system="You are an expert web scraping assistant. Return ONLY valid JSON, no extra text.",
        messages=[{"role": "user", "content": prompt}]
    )

    result = response.content[0].text
    if "```json" in result:
        result = result.split("```json")[1].split("```")[0].strip()
    elif "```" in result:
        result = result.split("```")[1].split("```")[0].strip()

    return json.loads(result)


def analyze_with_ollama(html: str, name: str, url: str, model: str = "llama3") -> dict:
    """Analyze HTML using local Ollama."""
    import requests

    prompt = ANALYSIS_PROMPT.format(name=name, url=url, html=html[:30000])

    response = requests.post(
        "http://localhost:11434/api/generate",
        json={
            "model": model,
            "prompt": f"Return ONLY valid JSON:\n{prompt}",
            "stream": False,
            "format": "json"
        }
    )

    result = response.json()["response"]
    return json.loads(result)

# ============================================================
# Rust Code Generation
# ============================================================

RUST_PARSER_TEMPLATE = '''// ============================================================
// Auto-generated by AI Parser Generator
// Bookmaker: {name}
// URL: {url}
// Generated: {date}
// Model: {model}
// Confidence: {confidence}%
// ============================================================

use crate::base::{{BookmakerParser, ParserResult}};
use crate::headless_helper::HeadlessChromeHelper;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use shared::odds::OddsType;
use shared::{{Event, Odd, Sport}};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{{debug, info, warn}};

/// {name} parser - generated by AI
/// URL: {url}
/// AI Confidence: {confidence}%
#[derive(Debug)]
pub struct {name_struct}Parser {{
    client: Arc<Client>,
    base_url: String,
    urls: Vec<String>,
}}

impl {name_struct}Parser {{
    pub fn new(client: Arc<Client>) -> Self {{
        Self {{
            client,
            base_url: "{base_url}".to_string(),
            urls: vec![
                "{url}/live/football",
                "{url}/football",
                "{url}/live/basketball",
                "{url}/live/hockey",
            ],
        }}
    }}

    /// AI-generated selectors
    const SELECTORS: &'static [(&'static str, &'static str)] = &[
        ("event_container", "{event_container}"),
        ("home_team", "{home_team}"),
        ("away_team", "{away_team}"),
        ("league", "{league}"),
        ("odds_container", "{odds_container}"),
        ("odds_1x2", "{odds_1x2}"),
        ("odds_total_over", "{odds_total_over}"),
        ("odds_total_under", "{odds_total_under}"),
        ("odds_handicap", "{odds_handicap}"),
        ("live_indicator", "{live_indicator}"),
    ];

    fn get_selector(name: &str) -> &'static str {{
        Self::SELECTORS.iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| *s)
            .unwrap_or("")
    }}

    fn parse_page_data(url: &str, is_live: bool) -> (Vec<Event>, Vec<Odd>) {{
        let mut events = Vec::new();
        let mut all_odds = Vec::new();
        let now = Utc::now();

        match HeadlessChromeHelper::new() {{
            Ok(helper) => {{
                match helper.navigate_and_wait(url, 3000) {{
                    Ok(tab) => {{
                        let _ = HeadlessChromeHelper::scroll_page(&tab);

                        // Extract events using AI-generated selectors
                        let js = format!(r#"
                            (function() {{
                                const results = [];
                                const containers = document.querySelectorAll('{container_sel}');

                                for (const container of containers) {{
                                    try {{
                                        const home = (container.querySelector('{home_sel}')?.textContent || '').trim();
                                        const away = (container.querySelector('{away_sel}')?.textContent || '').trim();
                                        const league = (container.querySelector('{league_sel}')?.textContent || '').trim();

                                        const oddsEls = container.querySelectorAll('{odds_sel}');
                                        const odds = [];
                                        for (const el of oddsEls) {{
                                            const val = parseFloat(el.textContent.trim().replace(',', '.'));
                                            if (val > 1.0 && val < 100) {{
                                                odds.push(val);
                                            }}
                                        }}

                                        if (home && away && odds.length >= 2) {{
                                            results.push({{ home, away, league, odds }});
                                        }}
                                    }} catch(e) {{}}
                                }}

                                return JSON.stringify(results);
                            }})()
                        "#,
                            container_sel = Self::get_selector("event_container"),
                            home_sel = Self::get_selector("home_team"),
                            away_sel = Self::get_selector("away_team"),
                            league_sel = Self::get_selector("league"),
                            odds_sel = Self::get_selector("odds_1x2"),
                        );

                        if let Some(json_val) = HeadlessChromeHelper::evaluate_json(&tab, &js) {{
                            if let Some(results_str) = json_val.as_str() {{
                                if let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(results_str) {{
                                    for item in parsed {{
                                        if let (Some(home), Some(away)) = (
                                            item.get("home").and_then(|v| v.as_str()),
                                            item.get("away").and_then(|v| v.as_str()),
                                        ) {{
                                            let event_id = format!("{slug}-{{}}-{{}}",
                                                home.replace(' ', "_").replace('/', "-"),
                                                away.replace(' ', "_").replace('/', "-"));

                                            let league_name = item.get("league")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(if is_live {{ "Live" }} else {{ "Prematch" }})
                                                .to_string();

                                            let event = Event {{
                                                id: event_id.clone(),
                                                sport: Sport::Football,
                                                league: league_name,
                                                home_team: home.to_string(),
                                                away_team: away.to_string(),
                                                start_time: None,
                                                is_live,
                                                bookmaker_slug: "{slug}".to_string(),
                                                raw_url: None,
                                                extra: HashMap::new(),
                                            }};
                                            events.push(event);

                                            if let Some(odds_arr) = item.get("odds").and_then(|v| v.as_array()) {{
                                                for (i, odd_val) in odds_arr.iter().enumerate() {{
                                                    if let Some(odd_num) = odd_val.as_f64() {{
                                                        if odd_num > 1.0 {{
                                                            let (selection, odds_type) = match i {{
                                                                0 => ("1", OddsType::Home),
                                                                1 if odds_arr.len() == 3 => ("X", OddsType::Draw),
                                                                _ => ("2", OddsType::Away),
                                                            }};
                                                            all_odds.push(Odd {{
                                                                id: format!("{{}}-{{}}", event_id, selection),
                                                                event_id: event_id.clone(),
                                                                bookmaker_slug: "{slug}".to_string(),
                                                                market: "1X2".to_string(),
                                                                selection: selection.to_string(),
                                                                odds: odd_num,
                                                                odds_type,
                                                                line: None,
                                                                timestamp: now,
                                                            }});
                                                        }}
                                                    }}
                                                }}
                                            }}
                                        }}
                                    }}
                                }}
                            }}
                        }}
                    }}
                    Err(e) => {{
                        debug!(url = url, error = %e, "{name}: navigation failed");
                    }}
                }}
            }}
            Err(e) => {{
                debug!(error = %e, "{name}: failed to create headless browser");
            }}
        }}

        (events, all_odds)
    }}
}}

#[async_trait]
impl BookmakerParser for {name_struct}Parser {{
    fn name(&self) -> &str {{ "{name}" }}
    fn slug(&self) -> &str {{ "{slug}" }}
    fn is_enabled(&self) -> bool {{ true }}

    async fn fetch_events(&self) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {{
        let mut all_events = Vec::new();
        for url in &self.urls {{
            let url = url.clone();
            let (events, _) = tokio::task::spawn_blocking(move || {{
                Self::parse_page_data(&url, url.contains("live"))
            }}).await.unwrap_or_default();
            all_events.extend(events);
        }}
        info!(count = all_events.len(), "{name} events parsed");
        Ok(all_events)
    }}

    async fn fetch_odds(&self, _event_id: &str) -> Result<Vec<Odd>, Box<dyn std::error::Error + Send + Sync>> {{
        let mut all_odds = Vec::new();
        for url in &self.urls {{
            let url = url.clone();
            let (_, odds) = tokio::task::spawn_blocking(move || {{
                Self::parse_page_data(&url, url.contains("live"))
            }}).await.unwrap_or_default();
            all_odds.extend(odds);
        }}
        Ok(all_odds)
    }}

    async fn fetch_all(&self) -> Result<ParserResult, Box<dyn std::error::Error + Send + Sync>> {{
        let start = std::time::Instant::now();
        let mut all_events = Vec::new();
        let mut all_odds = Vec::new();

        for url in &self.urls {{
            let url = url.clone();
            let (events, odds) = tokio::task::spawn_blocking(move || {{
                Self::parse_page_data(&url, url.contains("live"))
            }}).await.unwrap_or_default();
            all_events.extend(events);
            all_odds.extend(odds);
        }}

        let elapsed = start.elapsed().as_millis() as u64;
        info!(events = all_events.len(), odds = all_odds.len(), time_ms = elapsed, "{name} fetch complete");
        Ok(ParserResult::new("{slug}", all_events, all_odds, elapsed))
    }}

    fn base_url(&self) -> &str {{ &self.base_url }}
    fn user_agent(&self) -> &str {{ "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" }}
}}
'''


def generate_rust_code(selectors: dict, name: str, url: str, model: str, confidence: float) -> str:
    """Generate Rust parser code from AI-discovered selectors."""
    # Extract base URL
    from urllib.parse import urlparse
    parsed = urlparse(url)
    base_url = f"{parsed.scheme}://{parsed.netloc}"

    # Create struct name (CamelCase)
    name_struct = ''.join(word.capitalize() for word in name.replace('-', ' ').replace('_', ' ').split())

    # Slug (snake_case)
    slug = name.lower().replace(' ', '_').replace('-', '_')

    # Fill template
    code = RUST_PARSER_TEMPLATE.format(
        name=name,
        name_struct=name_struct,
        url=url,
        base_url=base_url,
        slug=slug,
        model=model,
        confidence=confidence,
        date=datetime.now().strftime('%Y-%m-%d'),
        event_container=selectors.get('event_container', ''),
        home_team=selectors.get('home_team', ''),
        away_team=selectors.get('away_team', ''),
        league=selectors.get('league', ''),
        odds_container=selectors.get('odds_container', ''),
        odds_1x2=selectors.get('odds_1x2', ''),
        odds_total_over=selectors.get('odds_total_over', ''),
        odds_total_under=selectors.get('odds_total_under', ''),
        odds_handicap=selectors.get('odds_handicap', ''),
        live_indicator=selectors.get('live_indicator', ''),
    )

    return code

# ============================================================
# Main
# ============================================================

def main():
    parser = argparse.ArgumentParser(
        description='AI Parser Generator for Bookmaker Websites',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --url "https://example-bk.ru/football" --name "NewBk" --model gpt-4o
  %(prog)s --url "https://example-bk.ru/football" --name "NewBk" --model claude
  %(prog)s --url "https://example-bk.ru/football" --name "NewBk" --model gemini
  %(prog)s --url "https://example-bk.ru/football" --name "NewBk" --model ollama
        """
    )

    parser.add_argument('--url', required=True, help='Bookmaker URL (e.g., https://bk.ru/football)')
    parser.add_argument('--name', required=True, help='Bookmaker name (e.g., NewBookmaker)')
    parser.add_argument('--model', choices=['gpt-4o', 'claude', 'gemini', 'ollama'], default='gemini',
                        help='AI model to use (default: gemini)')
    parser.add_argument('--api-key', help='API key (not needed for ollama)')
    parser.add_argument('--output', help='Output file path (default: ../crates/parsers/src/{name}.rs)')
    parser.add_argument('--wait', type=int, default=5, help='Seconds to wait for JS rendering')
    parser.add_argument('--no-scroll', action='store_true', help='Disable auto-scrolling')
    parser.add_argument('--dry-run', action='store_true', help='Skip Playwright, use mock HTML')

    args = parser.parse_args()

    print("=" * 60)
    print("🤖 AI Parser Generator")
    print("=" * 60)
    print(f"📋 Bookmaker: {args.name}")
    print(f"🌐 URL: {args.url}")
    print(f"🧠 Model: {args.model}")
    print(f"⏱️  Wait: {args.wait}s")
    print()

    # Step 1: Extract HTML
    print("📄 Step 1: Extracting HTML structure...")
    if args.dry_run:
        html = "<html><body>Dry run mode</body></html>"
        print("   ⏭️  Skipped (dry-run mode)")
    else:
        html = extract_html(args.url, args.wait, not args.no_scroll)
        if html and not html.startswith("<html>"):
            html_data = json.loads(html)
            print(f"   ✅ Found {len(html_data)} structured elements")
        else:
            print("   ⚠️  Limited HTML extracted")

    # Step 2: Analyze with LLM
    print(f"\n🧠 Step 2: Analyzing with {args.model}...")
    api_key = args.api_key or os.environ.get('OPENAI_API_KEY') or os.environ.get('GOOGLE_API_KEY') or os.environ.get('ANTHROPIC_API_KEY', '')

    try:
        if args.model == 'gpt-4o':
            selectors = analyze_with_gpt4(html, args.name, args.url, api_key)
        elif args.model == 'gemini':
            selectors = analyze_with_gemini(html, args.name, args.url, api_key)
        elif args.model == 'claude':
            selectors = analyze_with_claude(html, args.name, args.url, api_key)
        elif args.model == 'ollama':
            selectors = analyze_with_ollama(html, args.name, args.url)
        else:
            raise ValueError(f"Unknown model: {args.model}")

        confidence = selectors.get('confidence', 0) * 100
        print(f"   ✅ Analysis complete (confidence: {confidence:.0f}%)")

        # Print found selectors
        print("\n📌 Found selectors:")
        for key, value in selectors.items():
            if key not in ('confidence', 'notes'):
                status = "✅" if value else "❌"
                print(f"   {status} {key}: {value or 'not found'}")

        if selectors.get('notes'):
            print(f"\n📝 Notes: {selectors['notes']}")

    except Exception as e:
        print(f"   ❌ LLM analysis failed: {e}")
        print("   Falling back to default template...")
        selectors = {
            'event_container': '[class*="event"], [class*="match"]',
            'home_team': '[class*="home"], [class*="team"]:first-child',
            'away_team': '[class*="away"], [class*="team"]:last-child',
            'league': '[class*="league"], [class*="tournament"]',
            'odds_container': '[class*="odds"], [class*="coeff"]',
            'odds_1x2': '[class*="1x2"], [class*="main"]',
            'odds_total_over': '[class*="over"], [class*="tb"]',
            'odds_total_under': '[class*="under"], [class*="tm"]',
            'odds_handicap': '[class*="handicap"], [class*="fora"]',
            'live_indicator': '[class*="live"]',
            'confidence': 0.3,
        }
        confidence = 30

    # Step 3: Generate Rust code
    print("\n⚙️  Step 3: Generating Rust parser...")
    output_path = args.output or f"../crates/parsers/src/{args.name.lower().replace(' ', '_')}.rs"

    code = generate_rust_code(selectors, args.name, args.url, args.model, confidence)

    # Save file
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(code)
    print(f"   ✅ Generated: {output_path}")
    print(f"   📊 Code size: {len(code):,} bytes")

    # Print next steps
    print("\n" + "=" * 60)
    print("📋 Next steps:")
    print("=" * 60)
    print(f"1. Add `pub mod {args.name.lower().replace(' ', '_')};` to lib.rs")
    print(f"2. Add to ParserFactory::new():")
    print(f'   parsers.insert("{args.name.lower()}", Arc::new({args.name.lower().replace(" ", "_")}::{args.name.replace(" ", "")}Parser::new(client.clone())));')
    print(f"3. Run `cargo build --package parsers` to verify")
    print(f"4. Test with `cargo run --bin test_parsers`")
    print()

    return 0


if __name__ == '__main__':
    sys.exit(main())
