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


def normalize_accept_language(value: str):
    normalized = " ".join(str(value).split()).strip()
    return normalized or None


def build_cookie_header(cookies):
    pairs = []
    seen = set()

    for cookie in cookies or []:
        if not isinstance(cookie, dict):
            continue
        name = str(cookie.get("name", "")).strip()
        value = str(cookie.get("value", "")).strip()
        if not name or not value:
            continue
        lowered = name.lower()
        if lowered in seen:
            continue
        seen.add(lowered)
        pairs.append(f"{name}={value}")

    return "; ".join(pairs) or None


def is_protection_cookie_name(name: str):
    lowered = str(name or "").strip().lower()
    return (
        lowered.startswith("qrator_")
        or lowered.startswith("__qrator")
        or lowered.startswith("qauth_")
        or lowered.startswith("qab")
    )


def summarize_runtime_status(cookies, header_profile=None, browser_verified_api_probe=None, direct_probe_status=0):
    cookie_names = []
    seen = set()

    for cookie in cookies or []:
        if not isinstance(cookie, dict):
            continue
        name = str(cookie.get("name", "")).strip()
        if not name:
            continue
        lowered = name.lower()
        if lowered in seen:
            continue
        seen.add(lowered)
        cookie_names.append(name)

    non_protection_cookie_count = sum(
        1 for name in cookie_names if not is_protection_cookie_name(name)
    )
    has_cookie_bootstrap = bool(cookie_names)
    has_browser_verified_api_probe = bool(
        browser_verified_api_probe
        and 200 <= int(browser_verified_api_probe.get("status") or 0) < 400
    )
    has_direct_probe_success = 200 <= int(direct_probe_status or 0) < 400
    has_validated_session_bootstrap = non_protection_cookie_count > 0
    can_attempt_runtime_with_bootstrap = (
        has_validated_session_bootstrap
        or (has_cookie_bootstrap and has_browser_verified_api_probe)
    )

    if has_validated_session_bootstrap:
        bootstrap_blocker = "ready"
    elif has_cookie_bootstrap and not has_browser_verified_api_probe:
        bootstrap_blocker = "protection_only_unverified_api"
    elif has_cookie_bootstrap:
        bootstrap_blocker = "protection_only"
    elif header_profile:
        bootstrap_blocker = "header_only"
    else:
        bootstrap_blocker = "bootstrap_unavailable"

    return {
        "bootstrap_blocker": bootstrap_blocker,
        "cookie_names": cookie_names,
        "cookie_count": len(cookie_names),
        "non_protection_cookie_count": non_protection_cookie_count,
        "protection_only": bool(has_cookie_bootstrap and non_protection_cookie_count == 0),
        "has_cookie_bootstrap": has_cookie_bootstrap,
        "has_validated_session_bootstrap": has_validated_session_bootstrap,
        "can_attempt_runtime_with_bootstrap": can_attempt_runtime_with_bootstrap,
        "has_browser_verified_api_probe": has_browser_verified_api_probe,
        "browser_verified_api_probe_status": (
            int(browser_verified_api_probe.get("status") or 0)
            if browser_verified_api_probe
            else 0
        ),
        "has_direct_probe_success": has_direct_probe_success,
        "direct_probe_status": int(direct_probe_status or 0),
    }


def preview_json(value, limit=2000):
    text = json.dumps(value, ensure_ascii=False, default=str)
    return text[:limit]


def summarize_api_request_headers(captured_requests):
    preferred_order = ["events_list", "filter", "tournament_tree", "action_lines"]
    request_by_kind = {}

    for request in captured_requests:
        endpoint_kind = request.get("endpoint_kind")
        if endpoint_kind and endpoint_kind not in request_by_kind:
            request_by_kind[endpoint_kind] = request

    for endpoint_kind in preferred_order:
        request = request_by_kind.get(endpoint_kind)
        if not request:
            continue
        headers = request.get("headers") or {}
        return {
            "Accept-Language": headers.get("accept-language") or headers.get("Accept-Language"),
            "Origin": headers.get("origin") or headers.get("Origin") or BASE_URL,
            "Referer": headers.get("referer") or headers.get("Referer") or ROOT_REFERER,
            "User-Agent": headers.get("user-agent") or headers.get("User-Agent"),
            "Sec-CH-UA": headers.get("sec-ch-ua") or headers.get("Sec-CH-UA"),
            "Sec-CH-UA-Mobile": headers.get("sec-ch-ua-mobile") or headers.get("Sec-CH-UA-Mobile"),
            "Sec-CH-UA-Platform": headers.get("sec-ch-ua-platform") or headers.get("Sec-CH-UA-Platform"),
            "X-Application-Name": headers.get("x-application-name") or headers.get("X-Application-Name"),
        }

    return None


def summarize_browser_verified_api_probe(captured_responses):
    preferred_order = ["events_list", "filter", "tournament_tree", "action_lines"]

    for endpoint_kind in preferred_order:
        for response in captured_responses:
            if response.get("capture_phase") != "navigation":
                continue
            if response.get("endpoint_kind") != endpoint_kind:
                continue
            status = int(response.get("status") or 0)
            if status < 200 or status >= 400:
                continue
            return {
                "endpoint_kind": endpoint_kind,
                "url": response.get("url"),
                "status": status,
                "content_type": response.get("content_type"),
                "body_length": response.get("body_length"),
                "timestamp": response.get("timestamp"),
            }

    return None


async def capture_storage_snapshot(page):
    return await page.evaluate(
        """
        () => {
            const collect = (storage) => {
                try {
                    const entries = [];
                    for (let index = 0; index < storage.length; index += 1) {
                        const key = storage.key(index);
                        if (!key) {
                            continue;
                        }
                        const value = storage.getItem(key);
                        entries.push({
                            key,
                            value,
                            valuePreview: typeof value === 'string' ? value.slice(0, 200) : null,
                            valueLength: typeof value === 'string' ? value.length : 0,
                        });
                    }
                    return entries;
                } catch (error) {
                    return [{ key: '__error__', value: String(error), valuePreview: String(error), valueLength: 0 }];
                }
            };

            return {
                localStorage: collect(window.localStorage),
                sessionStorage: collect(window.sessionStorage),
            };
        }
        """
    )


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


def build_header_profile(final_url: str, direct_probe_status: int, api_headers=None, browser_verified_api_probe=None):
    accept_language = normalize_accept_language("ru-RU,ru;q=0.9,en;q=0.8")
    api_headers = {key: value for key, value in (api_headers or {}).items() if value}
    return {
        "accept_language": accept_language,
        "origin": BASE_URL,
        "referer": final_url or ROOT_REFERER,
        "final_url": final_url,
        "browser_verified_api_probe": browser_verified_api_probe,
        "browser_verified_api_probe_status": (
            browser_verified_api_probe.get("status") if browser_verified_api_probe else 0
        ),
        "direct_probe_status": direct_probe_status,
        "api_headers": api_headers,
        "extraHTTPHeaders": {
            "Accept-Language": accept_language,
            "Origin": BASE_URL,
            "Referer": final_url or ROOT_REFERER,
        },
    }


ROOT_REFERER = f"{BASE_URL}/"


async def run_discovery(headless: bool, wait_after_nav: float, output_dir: Path, cookies_path: Path):
    output_dir.mkdir(parents=True, exist_ok=True)
    captured_requests = []
    captured_responses = []
    capture_phase = "navigation"

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
                "endpoint_kind": next((name for name, endpoint in KNOWN_ENDPOINTS.items() if url.startswith(endpoint)), None),
                "capture_phase": capture_phase,
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
                    "capture_phase": capture_phase,
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
        capture_phase = "direct_probe"

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
        storage_snapshot = await capture_storage_snapshot(page)
        cookies = await context.cookies()
        playwright_storage_state = await context.storage_state()
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
        "browser_storage": storage_snapshot,
        "requests": captured_requests,
        "responses": captured_responses,
        "cookies": cookies,
    }

    output_file = output_dir / f"ligastavok_discovery_{timestamp}.json"
    output_file.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")

    latest_file = output_dir / "ligastavok_discovery_latest.json"
    latest_file.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")

    repo_root = Path(__file__).resolve().parents[1]
    storage_state = dict(playwright_storage_state or {})
    storage_state["cookies"] = cookies
    storage_state["cookieHeader"] = build_cookie_header(cookies)
    if not storage_state.get("origins"):
        storage_state["origins"] = [
            {
                "origin": BASE_URL,
                "localStorage": [
                    {
                        "name": "i18nextLng",
                        "value": "ru-RU",
                    }
                ],
            }
        ]
    storage_state_file = repo_root / "ligastavok_storage_state.json"
    storage_state_file.write_text(
        json.dumps(storage_state, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    api_headers = summarize_api_request_headers(captured_requests)
    browser_verified_api_probe = summarize_browser_verified_api_probe(captured_responses)
    header_profile = build_header_profile(
        final_url,
        direct_probe.get("status", 0),
        api_headers,
        browser_verified_api_probe,
    )
    status = summarize_runtime_status(
        cookies,
        header_profile=header_profile,
        browser_verified_api_probe=browser_verified_api_probe,
        direct_probe_status=direct_probe.get("status", 0),
    )
    header_profile_file = repo_root / "ligastavok_header_profile.json"
    header_profile_file.write_text(
        json.dumps(header_profile, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    bootstrap_bundle = {
        "bookmaker": "ligastavok",
        "captured_at": result["captured_at"],
        "source": str(latest_file),
        "storageState": storage_state,
        "headerProfile": header_profile,
        "runtimeBootstrap": {
            "api_headers": api_headers,
            "browser_storage": storage_snapshot,
            "browser_verified_api_probe": browser_verified_api_probe,
            "browser_verified_api_probe_status": (
                browser_verified_api_probe.get("status") if browser_verified_api_probe else 0
            ),
            "direct_probe_status": direct_probe.get("status", 0),
        },
        "status": status,
        "final_url": final_url,
        "cookies": cookies,
        "cookieHeader": storage_state["cookieHeader"],
    }
    bootstrap_bundle_file = repo_root / "ligastavok_bootstrap.json"
    bootstrap_bundle_file.write_text(
        json.dumps(bootstrap_bundle, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    summary = {
        "output_file": str(output_file),
        "latest_file": str(latest_file),
        "storage_state_file": str(storage_state_file),
        "header_profile_file": str(header_profile_file),
        "bootstrap_bundle_file": str(bootstrap_bundle_file),
        "page_title": title,
        "final_url": final_url,
        "cookies_loaded": cookies_loaded,
        "captured_request_count": len(captured_requests),
        "captured_response_count": len(captured_responses),
        "browser_verified_api_probe_status": (
            browser_verified_api_probe.get("status", 0) if browser_verified_api_probe else 0
        ),
        "direct_probe_status": direct_probe.get("status", 0),
        "bootstrap_blocker": status["bootstrap_blocker"],
        "can_attempt_runtime_with_bootstrap": status["can_attempt_runtime_with_bootstrap"],
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
