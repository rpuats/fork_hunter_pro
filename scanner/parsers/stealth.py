# scanner/parsers/stealth.py
"""
Shared stealth module for all Playwright parsers.
Provides randomized fingerprinting to avoid detection.
"""
import asyncio
import random
import re
import time
from typing import Dict, List, Optional, Tuple
from playwright.async_api import async_playwright, Browser, BrowserContext, Page
from playwright_stealth import Stealth

USER_AGENTS = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
]

VIEWPORTS = [
    (1920, 1080),
    (1366, 768),
    (1536, 864),
    (1440, 900),
    (1600, 900),
    (1280, 720),
    (2560, 1440),
]

TIMEZONES = [
    "Europe/Moscow",
    "Europe/Samara",
    "Europe/Volgograd",
    "Asia/Yekaterinburg",
    "Asia/Omsk",
    "Asia/Krasnoyarsk",
]

LOCALES = [
    "ru-RU",
    "ru",
    "en-US",
    "en",
]

WEBGL_RENDERERS = [
    "Intel Iris OpenGL Engine",
    "Intel Iris Pro OpenGL Engine",
    "Intel HD Graphics 630",
    "Intel HD Graphics 620",
    "ANGLE (Intel, Intel(R) HD Graphics 630 Direct3D11 vs_5_0 ps_5_0)",
    "ANGLE (NVIDIA, NVIDIA GeForce GTX 1060 Direct3D11 vs_5_0 ps_5_0)",
    "ANGLE (AMD, AMD Radeon RX 580 Direct3D11 vs_5_0 ps_5_0)",
]

WEBGL_VENDORS = [
    "Intel Inc.",
    "Google Inc. (NVIDIA)",
    "Google Inc. (AMD)",
    "Intel",
    "NVIDIA",
    "AMD",
]


def random_user_agent() -> str:
    return random.choice(USER_AGENTS)


def random_viewport() -> Tuple[int, int]:
    return random.choice(VIEWPORTS)


def random_timezone() -> str:
    return random.choice(TIMEZONES)


def random_locale() -> str:
    return random.choice(LOCALES)


def random_webgl_config() -> Dict[str, str]:
    return {
        "renderer": random.choice(WEBGL_RENDERERS),
        "vendor": random.choice(WEBGL_VENDORS),
    }


def random_hardware_concurrency() -> int:
    return random.choice([4, 8, 12, 16])


def generate_stealth_config() -> Dict:
    ua = random_user_agent()
    viewport = random_viewport()
    tz = random_timezone()
    locale = random_locale()
    webgl = random_webgl_config()
    cores = random_hardware_concurrency()
    
    return {
        "user_agent": ua,
        "viewport": {"width": viewport[0], "height": viewport[1]},
        "timezone": tz,
        "locale": locale,
        "webgl_renderer": webgl["renderer"],
        "webgl_vendor": webgl["vendor"],
        "hardware_concurrency": cores,
        "languages": (locale, "en"),
    }


def parse_proxy(proxy_str: str) -> Optional[Dict]:
    """Parse proxy string into Playwright proxy config.
    
    Supported formats:
    - http://user:pass@host:port
    - https://user:pass@host:port
    - socks5://user:pass@host:port
    - socks5h://user:pass@host:port
    - host:port (defaults to http)
    """
    proxy_str = proxy_str.strip()
    if not proxy_str or proxy_str.startswith('#'):
        return None
    
    pattern = r'^(?P<scheme>https?|socks5h?)://(?:(?P<user>[^:]+):(?P<pass>[^@]+)@)?(?P<host>[^:]+):(?P<port>\d+)$'
    match = re.match(pattern, proxy_str)
    
    if match:
        scheme = match.group('scheme')
        server_scheme = 'http' if scheme in ('http', 'https') else scheme
        server = f"{server_scheme}://{match.group('host')}:{match.group('port')}"
        proxy_config = {"server": server}
        if match.group('user'):
            proxy_config["username"] = match.group('user')
        if match.group('pass'):
            proxy_config["password"] = match.group('pass')
        return proxy_config
    
    simple_pattern = r'^(?P<host>[^:]+):(?P<port>\d+)$'
    match = re.match(simple_pattern, proxy_str)
    if match:
        return {"server": f"http://{match.group('host')}:{match.group('port')}"}
    
    return None


async def check_proxy_health(proxy_url: str, timeout: float = 10.0) -> bool:
    """Check if proxy is working by making a test request."""
    proxy_config = parse_proxy(proxy_url)
    if not proxy_config:
        return False
    
    try:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(headless=True)
        context = await browser.new_context(proxy=proxy_config)
        page = await context.new_page()
        
        try:
            await page.goto('http://httpbin.org/ip', timeout=int(timeout * 1000))
            content = await page.content()
            is_healthy = 'origin' in content.lower() or page.url != 'http://httpbin.org/ip'
            return is_healthy
        except Exception:
            return False
        finally:
            await context.close()
            await browser.close()
            await pw.stop()
    except Exception:
        return False


async def get_working_proxies(proxy_list: List[str], timeout: float = 10.0) -> List[str]:
    """Filter proxy list and return only working proxies."""
    working = []
    tasks = []
    
    for proxy_url in proxy_list:
        tasks.append(check_proxy_health(proxy_url, timeout))
    
    results = await asyncio.gather(*tasks, return_exceptions=True)
    
    for proxy_url, result in zip(proxy_list, results):
        if isinstance(result, bool) and result:
            working.append(proxy_url)
    
    return working


async def create_stealth_context(
    browser: Browser,
    config: Optional[Dict] = None,
    proxy_list: Optional[List[str]] = None,
) -> BrowserContext:
    if config is None:
        config = generate_stealth_config()
    
    proxy_config = None
    if proxy_list and len(proxy_list) > 0:
        proxy_str = random.choice(proxy_list)
        proxy_config = parse_proxy(proxy_str)
    
    stealth = Stealth(
        chrome_app=True,
        chrome_csi=True,
        chrome_load_times=True,
        chrome_runtime=True,
        hairline=True,
        iframe_content_window=True,
        media_codecs=True,
        navigator_hardware_concurrency=True,
        navigator_languages=True,
        navigator_permissions=True,
        navigator_platform=True,
        navigator_plugins=True,
        navigator_user_agent=True,
        navigator_vendor=True,
        navigator_webdriver=True,
        error_prototype=True,
        sec_ch_ua=True,
        webgl_vendor=True,
        navigator_languages_override=config.get("languages", ("ru-RU", "ru")),
        navigator_platform_override="Win32",
        navigator_user_agent_override=config.get("user_agent"),
        webgl_renderer_override=config.get("webgl_renderer"),
        webgl_vendor_override=config.get("webgl_vendor"),
    )
    
    context_kwargs = {
        "user_agent": config.get("user_agent"),
        "viewport": config.get("viewport"),
        "locale": config.get("locale", "ru-RU"),
        "timezone_id": config.get("timezone", "Europe/Moscow"),
        "extra_http_headers": {
            "Accept-Language": f"{config.get('locale', 'ru-RU')},en-US;q=0.9,en;q=0.8",
        },
    }
    
    if proxy_config:
        context_kwargs["proxy"] = proxy_config
    
    context = await browser.new_context(**context_kwargs)
    
    await stealth.apply_stealth_async(context)
    
    tz_offset_minutes = int(time.timezone / 60)
    hw_cores = config.get('hardware_concurrency', 8)
    device_mem = random.choice([4, 8, 16])
    vp = config.get('viewport', {'width': 1920, 'height': 1080})
    webgl_vendor = config.get('webgl_vendor', 'Intel Inc.')
    webgl_renderer = config.get('webgl_renderer', 'Intel Iris OpenGL Engine')
    vp_width = vp.get('width', 1920) if isinstance(vp, dict) else vp[0]
    vp_height = vp.get('height', 1080) if isinstance(vp, dict) else vp[1]
    
    init_script = f"""
        () => {{
            const getParameter = WebGLRenderingContext.prototype.getParameter;
            
            WebGLRenderingContext.prototype.getParameter = function(parameter) {{
                if (parameter === 37445) return '{webgl_vendor}';
                if (parameter === 37446) return '{webgl_renderer}';
                if (parameter === 35724) return 'WebGL 1.0';
                return getParameter.call(this, parameter);
            }};
            
            const toDataURL = HTMLCanvasElement.prototype.toDataURL;
            HTMLCanvasElement.prototype.toDataURL = function(type) {{
                if (type === 'image/png') {{
                    const ctx = this.getContext('2d');
                    if (ctx) {{
                        const imageData = ctx.getImageData(0, 0, this.width, this.height);
                        for (let i = 0; i < imageData.data.length; i += 4) {{
                            imageData.data[i] = Math.min(255, imageData.data[i] + Math.random() * 2 - 1);
                            imageData.data[i+1] = Math.min(255, imageData.data[i+1] + Math.random() * 2 - 1);
                            imageData.data[i+2] = Math.min(255, imageData.data[i+2] + Math.random() * 2 - 1);
                        }}
                        ctx.putImageData(imageData, 0, 0);
                    }}
                }}
                return toDataURL.call(this, type);
            }};
            
            const originalRTCPeerConnection = window.RTCPeerConnection;
            if (originalRTCPeerConnection) {{
                window.RTCPeerConnection = function(...args) {{
                    const pc = new originalRTCPeerConnection(...args);
                    const originalCreateOffer = pc.createOffer;
                    pc.createOffer = async function(...offerArgs) {{
                        const offer = await originalCreateOffer.apply(this, offerArgs);
                        offer.sdp = offer.sdp.replace(/a=fingerprint:sha-256:.*/g, 'a=fingerprint:sha-256:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00');
                        return offer;
                    }};
                    return pc;
                }};
            }}
            
            Date.prototype.getTimezoneOffset = function() {{
                return {tz_offset_minutes};
            }};
            
            Object.defineProperty(navigator, 'hardwareConcurrency', {{
                get: () => {hw_cores}
            }});
            
            Object.defineProperty(navigator, 'deviceMemory', {{
                get: () => {device_mem}
            }});
            
            Object.defineProperty(screen, 'width', {{ get: () => {vp_width} }});
            Object.defineProperty(screen, 'height', {{ get: () => {vp_height} }});
            Object.defineProperty(screen, 'availWidth', {{ get: () => {vp_width} }});
            Object.defineProperty(screen, 'availHeight', {{ get: () => {vp_height} }});
            Object.defineProperty(window, 'innerWidth', {{ get: () => {vp_width} }});
            Object.defineProperty(window, 'innerHeight', {{ get: () => {vp_height} }});
            Object.defineProperty(window, 'outerWidth', {{ get: () => {vp_width} }});
            Object.defineProperty(window, 'outerHeight', {{ get: () => {vp_height} }});
            
            if (window.performance && window.performance.timing) {{
                const timing = window.performance.timing;
                Object.defineProperty(timing, 'navigationStart', {{ get: () => Date.now() - Math.random() * 5000 }});
            }}
            
            delete window.cdc_adoQpoasnfa76pfcZLmcfl_Array;
            delete window.cdc_adoQpoasnfa76pfcZLmcfl_Promise;
            delete window.cdc_adoQpoasnfa76pfcZLmcfl_Symbol;
        }}
    """
    
    await context.add_init_script(init_script)
    
    return context


async def launch_stealth_browser(
    headless: bool = True,
    config: Optional[Dict] = None,
) -> Tuple[Browser, BrowserContext]:
    pw = await async_playwright().start()
    
    browser = await pw.chromium.launch(
        headless=headless,
        args=[
            '--disable-blink-features=AutomationControlled',
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--disable-web-security',
            '--disable-features=IsolateOrigins,site-per-process',
            '--disable-infobars',
            '--window-size=1920,1080',
            '--lang=ru-RU',
        ],
    )
    
    context = await create_stealth_context(browser, config)
    
    return browser, context


async def apply_stealth(
    page_or_context,
    config: Optional[Dict] = None,
) -> None:
    if config is None:
        config = generate_stealth_config()
    
    stealth = Stealth(
        chrome_app=True,
        chrome_csi=True,
        chrome_load_times=True,
        chrome_runtime=True,
        hairline=True,
        iframe_content_window=True,
        media_codecs=True,
        navigator_hardware_concurrency=True,
        navigator_languages=True,
        navigator_permissions=True,
        navigator_platform=True,
        navigator_plugins=True,
        navigator_user_agent=True,
        navigator_vendor=True,
        navigator_webdriver=True,
        error_prototype=True,
        sec_ch_ua=True,
        webgl_vendor=True,
        navigator_languages_override=config.get("languages", ("ru-RU", "ru")),
        navigator_platform_override="Win32",
        navigator_user_agent_override=config.get("user_agent"),
        webgl_renderer_override=config.get("webgl_renderer"),
        webgl_vendor_override=config.get("webgl_vendor"),
    )
    
    await stealth.apply_stealth_async(page_or_context)