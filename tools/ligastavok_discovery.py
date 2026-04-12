#!/usr/bin/env python3
"""Liga Stavok discovery helper.

Focused Playwright-based discovery for Liga Stavok's protected sports API.
Captures request metadata, request bodies, and JSON response previews for the
known sportsbook endpoints behind QRATOR.
"""

import argparse
import asyncio
import json
import time
import uuid
from datetime import datetime
from pathlib import Path

from playwright.async_api import async_playwright


BASE_URL = "https://www.ligastavok.ru"
API_HOST = "https://lds-api-sites.ligastavok.ru"
KNOWN_ENDPOINTS = {
    "events_list": f"{API_HOST}/rest/events/v8/eventsList",
    "action_lines": f"{API_HOST}/rest/events/v8/actionLines",
    "filter": f"{API_HOST}/rest/events/v2/filter",
    "tournament_tree": f"{API_HOST}/rest/events/v8/tournamentTree",
}
PAGE_TARGETS = [
    f"{BASE_URL}/live/football",
    f"{BASE_URL}/line/football",
]
DEFAULT_PAYLOAD = {
    "gameId": [],
    "limit": 200,
    "skip": 0,
    "topEvents": False,
    "view": "priority",
    "widgetVideo": False,
    "proposedTypes": ["MAINOFFER"],
}


def preview_json(value, limit=2000):
    text = json.dumps(value, ensure_ascii=False, default=str)
    return text[:limit]


async def maybe_load_cookies(context, cookies_path: Path):
    if not cookies_path.exists():
        return False

    try:
        cookies = json.loads(cookies_path.read_text(encoding="utf-8"))
        if isinstance(cookies, list) and cookies:
            await context.add_cookies(cookies)
            return True
    except Exception:
        return False

    return False


async def run_discovery(headless: bool, wait_after_nav: float, output_dir: Path, cookies_path: Path):
    output_dir.mkdir(parents=True, exist_ok=True)
    captured_requests = []
    captured_responses = []

    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(
            headless=headless,
            args=[
                "--disable-blink-features=AutomationControlled",
                "--no-sandbox",
                "--disable-dev-shm-usage",
            ],
        )
        context = await browser.new_context(
            user_agent=(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                "AppleWebKit/537.36 (KHTML, like Gecko) "
                "Chrome/131.0.0.0 Safari/537.36"
            ),
            viewport={"width": 1600, "height": 1000},
            locale="ru-RU",
            timezone_id="Europe/Moscow",
        )
        cookies_loaded = await maybe_load_cookies(context, cookies_path)
        page = await context.new_page()

        async def on_response(response):
            url = response.url
            if "ligastavok" not in url and "sportsapi.ru" not in url:
                return

            request = response.request
            post_data = None
            try:
                post_data = request.post_data
            except Exception:
                pass

            request_info = {
                "url": url,
                "method": request.method,
                "resource_type": request.resource_type,
                "headers": await request.all_headers(),
                "post_data": post_data,
                "timestamp": datetime.now().isoformat(),
            }
            captured_requests.append(request_info)

            body_preview = None
            body_length = 0
            content_type = response.headers.get("content-type", "")
            try:
                if "json" in content_type:
                    payload = await response.json()
                    body_preview = preview_json(payload)
                    body_length = len(json.dumps(payload, ensure_ascii=False, default=str))
                else:
                    text = await response.text()
                    body_preview = text[:1000]
                    body_length = len(text)
            except Exception:
                pass

            endpoint_kind = next((name for name, endpoint in KNOWN_ENDPOINTS.items() if url.startswith(endpoint)), None)
            captured_responses.append(
                {
                    "url": url,
                    "endpoint_kind": endpoint_kind,
                    "status": response.status,
                    "content_type": content_type,
                    "body_length": body_length,
                    "body_preview": body_preview,
                    "request_method": request.method,
                    "request_post_data": post_data,
                    "timestamp": datetime.now().isoformat(),
                }
            )

        page.on("response", on_response)

        for target in PAGE_TARGETS:
            try:
                await page.goto(target, wait_until="domcontentloaded", timeout=45000)
                await page.wait_for_timeout(int(wait_after_nav * 1000))
                await page.mouse.move(400, 300)
                await page.mouse.move(700, 500)
                await page.wait_for_timeout(1000)
            except Exception as exc:
                captured_responses.append(
                    {
                        "url": target,
                        "endpoint_kind": "navigation_error",
                        "status": 0,
                        "content_type": "",
                        "body_length": 0,
                        "body_preview": str(exc),
                        "request_method": "GET",
                        "request_post_data": None,
                        "timestamp": datetime.now().isoformat(),
                    }
                )

        direct_probe_payload = dict(DEFAULT_PAYLOAD)
        direct_probe_payload["ts"] = int(time.time() * 1000)
        direct_probe_headers = {
            "Content-Type": "application/json",
            "x-application-name": "mobile",
            "x-req-id": str(uuid.uuid4()),
        }

        direct_probe = await page.evaluate(
            """
            async ({ url, headers, payload }) => {
                try {
                    const response = await fetch(url, {
                        method: 'POST',
                        headers,
                        body: JSON.stringify(payload),
                    });
                    const text = await response.text();
                    return {
                        ok: response.ok,
                        status: response.status,
                        text: text.slice(0, 2000),
                    };
                } catch (error) {
                    return { ok: false, status: 0, text: String(error) };
                }
            }
            """,
            {
                "url": KNOWN_ENDPOINTS["events_list"],
                "headers": direct_probe_headers,
                "payload": direct_probe_payload,
            },
        )

        await page.wait_for_timeout(3000)
        title = await page.title()
        final_url = page.url
        cookies = await context.cookies()
        await browser.close()

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    result = {
        "bookmaker": "ligastavok",
        "captured_at": datetime.now().isoformat(),
        "headless": headless,
        "cookies_loaded": cookies_loaded,
        "page_title": title,
        "final_url": final_url,
        "known_endpoints": KNOWN_ENDPOINTS,
        "direct_probe": {
            "endpoint": KNOWN_ENDPOINTS["events_list"],
            "headers": direct_probe_headers,
            "payload": direct_probe_payload,
            "result": direct_probe,
        },
        "requests": captured_requests,
        "responses": captured_responses,
        "cookies": cookies,
    }

    output_file = output_dir / f"ligastavok_discovery_{timestamp}.json"
    output_file.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")

    summary = {
        "output_file": str(output_file),
        "page_title": title,
        "final_url": final_url,
        "cookies_loaded": cookies_loaded,
        "captured_request_count": len(captured_requests),
        "captured_response_count": len(captured_responses),
        "direct_probe_status": direct_probe.get("status", 0),
    }
    print(json.dumps(summary, ensure_ascii=False, indent=2))


def main():
    parser = argparse.ArgumentParser(description="Focused Liga Stavok discovery helper")
    parser.add_argument("--headless", action="store_true", help="Run browser in headless mode")
    parser.add_argument("--wait-after-nav", type=float, default=12.0, help="Seconds to wait after each navigation")
    parser.add_argument(
        "--output-dir",
        default=str(Path(__file__).resolve().parent / "discovery_output" / "ligastavok"),
        help="Directory for discovery artifacts",
    )
    parser.add_argument(
        "--cookies",
        default=str(Path(__file__).resolve().parents[1] / "ligastavok_cookies.json"),
        help="Optional cookies JSON exported from a prior Liga Stavok session",
    )
    args = parser.parse_args()

    asyncio.run(
        run_discovery(
            headless=args.headless,
            wait_after_nav=args.wait_after_nav,
            output_dir=Path(args.output_dir),
            cookies_path=Path(args.cookies),
        )
    )


if __name__ == "__main__":
    main()
