"""Visible Playwright account login helper for Fork Hunter.

The Rust API sends the request JSON over stdin so credentials do not appear in
process arguments. The script opens a persistent Chromium profile, fills the
best-effort login/password fields, waits for the operator to complete captcha or
2FA, then saves storage_state and returns a redacted session summary.
"""

from __future__ import annotations

import asyncio
import json
import re
import sys
import time
from pathlib import Path
from typing import Any

from playwright.async_api import TimeoutError as PlaywrightTimeoutError
from playwright.async_api import async_playwright


LOGIN_URLS = {
    "pari": "https://pari.ru/",
    "fonbet": "https://www.fon.bet/",
    "marathon": "https://www.marathonbet.ru/",
    "zenit": "https://zenit.win/",
    "betcity": "https://betcity.ru/",
    "baltbet": "https://www.baltbet.ru/",
    "bettery": "https://bettery.ru/",
    "leon": "https://leon.ru/",
    "sportbet": "https://sportbet.ru/",
    "bet24": "https://24betting.ru/",
}

LOGIN_SELECTORS = [
    "input[autocomplete='username']",
    "input[autocomplete='tel']",
    "input[type='tel']",
    "input[type='email']",
    "input[name*='login' i]",
    "input[name*='phone' i]",
    "input[name*='email' i]",
    "input[id*='login' i]",
    "input[id*='phone' i]",
    "input[placeholder*='тел' i]",
    "input[placeholder*='phone' i]",
    "input[placeholder*='email' i]",
    "input[placeholder*='логин' i]",
]

PASSWORD_SELECTORS = [
    "input[autocomplete='current-password']",
    "input[type='password']",
    "input[name*='pass' i]",
    "input[id*='pass' i]",
    "input[placeholder*='парол' i]",
    "input[placeholder*='pass' i]",
]

SUBMIT_SELECTORS = [
    "button[type='submit']",
    "input[type='submit']",
    "button:has-text('Войти')",
    "button:has-text('Вход')",
    "button:has-text('Login')",
    "button:has-text('Sign in')",
    "text=Войти",
]

OPEN_LOGIN_SELECTORS = [
    "button:has-text('Войти')",
    "a:has-text('Войти')",
    "button:has-text('Вход')",
    "a:has-text('Вход')",
    "button:has-text('Login')",
    "a:has-text('Login')",
]

AUTH_TEXT_PATTERNS = [
    re.compile(pattern, re.I)
    for pattern in [
        r"баланс",
        r"пополнить",
        r"вывести",
        r"личн\w*\s+кабинет",
        r"мой\s+профиль",
        r"account",
        r"deposit",
        r"withdraw",
    ]
]

BALANCE_RE = re.compile(r"(?:баланс|balance)[^\d]{0,40}([0-9][0-9\s.,]{1,18})", re.I)


def eprint(message: str) -> None:
    print(message, file=sys.stderr, flush=True)


def first_text_match_balance(text: str) -> str | None:
    match = BALANCE_RE.search(text)
    if not match:
        return None
    return match.group(1).strip()


def cookie_header_from_storage(storage_state: dict[str, Any]) -> str | None:
    cookies = storage_state.get("cookies") or []
    pairs = []
    for cookie in cookies:
        name = str(cookie.get("name") or "").strip()
        value = str(cookie.get("value") or "").strip()
        if name and value:
            pairs.append(f"{name}={value}")
    return "; ".join(pairs) if pairs else None


async def click_first_visible(page, selectors: list[str], timeout_ms: int = 900) -> bool:
    for selector in selectors:
        try:
            locator = page.locator(selector).first
            if await locator.count() == 0:
                continue
            await locator.click(timeout=timeout_ms)
            return True
        except Exception:
            continue
    return False


async def fill_first_visible(page, selectors: list[str], value: str, timeout_ms: int = 1200) -> bool:
    for selector in selectors:
        try:
            locator = page.locator(selector).first
            if await locator.count() == 0:
                continue
            await locator.fill(value, timeout=timeout_ms)
            return True
        except Exception:
            continue
    return False


async def visible_password_present(page) -> bool:
    for selector in PASSWORD_SELECTORS:
        try:
            locator = page.locator(selector).first
            if await locator.count() > 0 and await locator.is_visible(timeout=250):
                return True
        except Exception:
            continue
    return False


async def auth_signal(page, context) -> tuple[bool, str | None, str]:
    text = ""
    try:
        text = await page.locator("body").inner_text(timeout=1000)
    except Exception:
        pass

    storage_state = await context.storage_state()
    cookie_count = len(storage_state.get("cookies") or [])
    origin_count = len(storage_state.get("origins") or [])
    password_present = await visible_password_present(page)
    text_authenticated = any(pattern.search(text) for pattern in AUTH_TEXT_PATTERNS)
    balance_text = first_text_match_balance(text)

    authenticated = (text_authenticated or balance_text is not None) and not password_present
    detail = f"cookies={cookie_count};origins={origin_count};password_form={password_present};text_auth={text_authenticated}"
    return authenticated, balance_text, detail


async def run_agent(payload: dict[str, Any]) -> dict[str, Any]:
    bookmaker = str(payload.get("bookmaker") or "pari").strip().lower()
    login = str(payload.get("login") or "").strip()
    password = str(payload.get("password") or "")
    if not login or not password:
        raise ValueError("login and password are required")

    login_url = str(payload.get("login_url") or LOGIN_URLS.get(bookmaker) or "https://www.google.com/")
    wait_timeout_secs = int(payload.get("wait_timeout_secs") or 240)
    wait_timeout_secs = max(15, min(wait_timeout_secs, 900))
    profile_dir = Path(str(payload.get("profile_dir") or f"data/account_profiles/{bookmaker}"))
    storage_state_path = Path(str(payload.get("storage_state_path") or f"data/account_sessions/{bookmaker}.json"))
    profile_dir.mkdir(parents=True, exist_ok=True)
    storage_state_path.parent.mkdir(parents=True, exist_ok=True)

    started_at = time.time()
    filled_login = False
    filled_password = False
    clicked_submit = False
    authenticated = False
    balance_text = None
    auth_detail = "not_checked"

    async with async_playwright() as pw:
        context = await pw.chromium.launch_persistent_context(
            str(profile_dir),
            headless=False,
            viewport={"width": 1360, "height": 900},
            locale="ru-RU",
            timezone_id="Europe/Moscow",
            args=[
                "--disable-blink-features=AutomationControlled",
                "--disable-dev-shm-usage",
            ],
        )
        await context.add_init_script(
            """
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            Object.defineProperty(navigator, 'languages', { get: () => ['ru-RU', 'ru', 'en-US', 'en'] });
            """
        )
        page = context.pages[0] if context.pages else await context.new_page()
        try:
            eprint(f"[{bookmaker}] opening {login_url}")
            await page.goto(login_url, wait_until="domcontentloaded", timeout=45000)
            await page.wait_for_timeout(2500)

            await click_first_visible(page, OPEN_LOGIN_SELECTORS, timeout_ms=1000)
            await page.wait_for_timeout(1500)

            filled_login = await fill_first_visible(page, LOGIN_SELECTORS, login)
            filled_password = await fill_first_visible(page, PASSWORD_SELECTORS, password)

            if filled_login and filled_password:
                clicked_submit = await click_first_visible(page, SUBMIT_SELECTORS, timeout_ms=1200)
                if not clicked_submit:
                    try:
                        await page.keyboard.press("Enter")
                        clicked_submit = True
                    except Exception:
                        pass

            eprint(f"[{bookmaker}] waiting for captcha/2FA/manual confirmation up to {wait_timeout_secs}s")
            deadline = time.time() + wait_timeout_secs
            while time.time() < deadline:
                authenticated, balance_text, auth_detail = await auth_signal(page, context)
                if authenticated:
                    break
                await page.wait_for_timeout(2500)

            storage_state = await context.storage_state(path=str(storage_state_path))
            cookie_header = cookie_header_from_storage(storage_state)
            result = {
                "bookmaker": bookmaker,
                "login": login,
                "authenticated": authenticated,
                "status": "authenticated" if authenticated else "manual_attention_required",
                "login_url": login_url,
                "profile_dir": str(profile_dir),
                "storage_state_path": str(storage_state_path),
                "cookie_header": cookie_header,
                "cookie_count": len(storage_state.get("cookies") or []),
                "origin_count": len(storage_state.get("origins") or []),
                "filled_login": filled_login,
                "filled_password": filled_password,
                "clicked_submit": clicked_submit,
                "balance_text": balance_text,
                "detail": auth_detail,
                "duration_secs": round(time.time() - started_at, 2),
            }
            return result
        finally:
            await context.close()


async def main() -> int:
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw or "{}")
        result = await run_agent(payload)
        print(json.dumps({"success": True, "data": result}, ensure_ascii=False), flush=True)
        return 0 if result.get("authenticated") else 2
    except PlaywrightTimeoutError as error:
        print(json.dumps({"success": False, "error": f"playwright timeout: {error}"}, ensure_ascii=False), flush=True)
        return 3
    except Exception as error:
        print(json.dumps({"success": False, "error": str(error)}, ensure_ascii=False), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
