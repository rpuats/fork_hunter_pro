"""Bulk sequential account login agent for Fork Hunter.

Processes multiple bookmakers sequentially:
1. Opens browser for first BK
2. Fills login/password (adds +7 for phone numbers if needed)
3. Waits for captcha/2FA/manual confirmation
4. Saves cookies, extracts balance
5. Closes browser
6. Moves to next BK

Usage via stdin JSON:
{
    "accounts": [
        {"bookmaker": "pari", "login": "+79991234567", "password": "pass1"},
        {"bookmaker": "fonbet", "login": "user@email.com", "password": "pass2"},
        {"bookmaker": "marathon", "login": "login3", "password": "pass3"}
    ],
    "wait_timeout_secs": 240,
    "auto_close_after_auth": true
}
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

PHONE_RE = re.compile(r"^(\+?7|8)?\s*\(?\d{3}\)?\s*\d{3}[\s-]?\d{2}[\s-]?\d{2}$")


def eprint(message: str) -> None:
    print(message, file=sys.stderr, flush=True)


def format_phone_for_bookmaker(login: str, bookmaker: str) -> str:
    """Format phone number based on bookmaker requirements."""
    login = login.strip()
    
    # Check if looks like phone number
    if not PHONE_RE.match(login):
        return login  # Not a phone, return as-is
    
    # Remove all non-digits
    digits = re.sub(r"\D", "", login)
    
    # Remove leading 8 or 7 if present
    if digits.startswith("8") and len(digits) == 11:
        digits = digits[1:]  # Remove leading 8
    elif digits.startswith("7") and len(digits) == 11:
        digits = digits[1:]  # Remove leading 7
    
    # Format based on bookmaker
    if bookmaker in ["pari", "bettery", "leon"]:
        # These expect +7 format
        return f"+7{digits}"
    elif bookmaker in ["fonbet", "marathon", "zenit"]:
        # These may expect just 10 digits or +7
        return f"+7{digits}"
    elif bookmaker in ["betcity", "baltbet"]:
        # These may expect 11 digits starting with 7
        return f"7{digits}"
    else:
        # Default: +7 format
        return f"+7{digits}"


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


async def process_single_bookmaker(
    bookmaker: str,
    login: str,
    password: str,
    wait_timeout_secs: int,
    auto_close: bool,
) -> dict[str, Any]:
    """Process single bookmaker authentication."""
    
    # Format login (especially phone numbers)
    formatted_login = format_phone_for_bookmaker(login, bookmaker)
    
    login_url = LOGIN_URLS.get(bookmaker) or "https://www.google.com/"
    profile_dir = Path(f"data/account_profiles/{bookmaker}/{formatted_login}")
    storage_state_path = Path(f"data/account_sessions/{bookmaker}/{formatted_login}.json")
    profile_dir.mkdir(parents=True, exist_ok=True)
    storage_state_path.parent.mkdir(parents=True, exist_ok=True)

    started_at = time.time()
    filled_login = False
    filled_password = False
    clicked_submit = False
    authenticated = False
    balance_text = None
    auth_detail = "not_checked"

    eprint(f"\n{'='*60}")
    eprint(f"[BULK] Processing {bookmaker.upper()}")
    eprint(f"[BULK] Login: {formatted_login}")
    eprint(f"[BULK] Opening browser...")
    eprint(f"{'='*60}")

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
            eprint(f"[{bookmaker}] Navigating to {login_url}")
            await page.goto(login_url, wait_until="domcontentloaded", timeout=45000)
            await page.wait_for_timeout(2500)

            # Click login button if needed
            await click_first_visible(page, OPEN_LOGIN_SELECTORS, timeout_ms=1000)
            await page.wait_for_timeout(1500)

            # Fill credentials
            eprint(f"[{bookmaker}] Filling login...")
            filled_login = await fill_first_visible(page, LOGIN_SELECTORS, formatted_login)
            
            eprint(f"[{bookmaker}] Filling password...")
            filled_password = await fill_first_visible(page, PASSWORD_SELECTORS, password)

            if filled_login and filled_password:
                eprint(f"[{bookmaker}] Clicking submit...")
                clicked_submit = await click_first_visible(page, SUBMIT_SELECTORS, timeout_ms=1200)
                if not clicked_submit:
                    try:
                        await page.keyboard.press("Enter")
                        clicked_submit = True
                    except Exception:
                        pass

            eprint(f"[{bookmaker}] ⏳ Waiting for captcha/2FA/manual confirmation (up to {wait_timeout_secs}s)")
            eprint(f"[{bookmaker}] 📝 Please complete captcha or 2FA in the browser window")
            
            deadline = time.time() + wait_timeout_secs
            last_status_print = 0
            
            while time.time() < deadline:
                authenticated, balance_text, auth_detail = await auth_signal(page, context)
                
                # Print status every 5 seconds
                if time.time() - last_status_print > 5:
                    status = "✅ AUTHENTICATED" if authenticated else "⏳ Waiting..."
                    eprint(f"[{bookmaker}] {status} (elapsed: {int(time.time() - started_at)}s)")
                    last_status_print = time.time()
                
                if authenticated:
                    eprint(f"[{bookmaker}] ✅ Successfully authenticated!")
                    if balance_text:
                        eprint(f"[{bookmaker}] 💰 Balance detected: {balance_text}")
                    break
                    
                await page.wait_for_timeout(2500)

            if not authenticated:
                eprint(f"[{bookmaker}] ⚠️ Timeout - manual attention required")

            # Save session
            storage_state = await context.storage_state(path=str(storage_state_path))
            cookie_header = cookie_header_from_storage(storage_state)
            
            result = {
                "bookmaker": bookmaker,
                "login": formatted_login,
                "raw_login": login,
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
            
            eprint(f"[{bookmaker}] {'✅ Done' if authenticated else '⚠️ Incomplete'} in {result['duration_secs']}s")
            return result
            
        finally:
            if auto_close:
                eprint(f"[{bookmaker}] Closing browser...")
            await context.close()


async def run_bulk_login(payload: dict[str, Any]) -> dict[str, Any]:
    """Process multiple bookmakers sequentially."""
    accounts = payload.get("accounts", [])
    if not accounts:
        raise ValueError("No accounts provided")
    
    wait_timeout_secs = int(payload.get("wait_timeout_secs", 240))
    auto_close = payload.get("auto_close_after_auth", True)
    
    results = []
    total = len(accounts)
    successful = 0
    failed = 0
    
    eprint(f"\n{'#'*70}")
    eprint(f"# BULK LOGIN AGENT - {total} accounts to process")
    eprint(f"# Timeout per account: {wait_timeout_secs}s")
    eprint(f"# Auto-close: {auto_close}")
    eprint(f"{'#'*70}\n")
    
    for idx, account in enumerate(accounts, 1):
        bookmaker = str(account.get("bookmaker", "")).strip().lower()
        login = str(account.get("login", "")).strip()
        password = str(account.get("password", ""))
        
        if not bookmaker or not login or not password:
            eprint(f"\n[{idx}/{total}] ⚠️ SKIPPED: Missing required fields")
            results.append({
                "bookmaker": bookmaker or "unknown",
                "login": login or "unknown",
                "status": "skipped",
                "error": "Missing bookmaker, login, or password",
                "authenticated": False,
            })
            failed += 1
            continue
        
        eprint(f"\n{'#'*70}")
        eprint(f"# [{idx}/{total}] Processing: {bookmaker.upper()}")
        eprint(f"{'#'*70}")
        
        try:
            result = await process_single_bookmaker(
                bookmaker, login, password, wait_timeout_secs, auto_close
            )
            results.append(result)
            if result.get("authenticated"):
                successful += 1
            else:
                failed += 1
        except Exception as e:
            eprint(f"\n[{idx}/{total}] ❌ ERROR processing {bookmaker}: {e}")
            results.append({
                "bookmaker": bookmaker,
                "login": login,
                "status": "error",
                "error": str(e),
                "authenticated": False,
            })
            failed += 1
        
        # Small delay between accounts to avoid rate limiting
        if idx < total:
            eprint(f"\n[{idx}/{total}] ⏳ Pausing 2s before next account...")
            await asyncio.sleep(2)
    
    eprint(f"\n{'#'*70}")
    eprint(f"# BULK LOGIN COMPLETE")
    eprint(f"# Total: {total} | Successful: {successful} | Failed: {failed}")
    eprint(f"{'#'*70}\n")
    
    return {
        "total": total,
        "successful": successful,
        "failed": failed,
        "results": results,
    }


async def main() -> int:
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw or "{}")
        result = await run_bulk_login(payload)
        
        # Output final result as JSON
        print(json.dumps({
            "success": True,
            "data": result
        }, ensure_ascii=False, indent=2), flush=True)
        
        return 0 if result["failed"] == 0 else 2
        
    except PlaywrightTimeoutError as error:
        print(json.dumps({
            "success": False,
            "error": f"playwright timeout: {error}"
        }, ensure_ascii=False), flush=True)
        return 3
    except Exception as error:
        print(json.dumps({
            "success": False,
            "error": str(error)
        }, ensure_ascii=False), flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(asyncio.run(main()))
