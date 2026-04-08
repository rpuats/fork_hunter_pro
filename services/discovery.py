# services/discovery.py
"""
API Discovery Service for Russian Bookmakers
Tries known endpoint patterns to find working APIs
"""
import asyncio
import aiohttp
import time
from typing import Dict, List, Optional, Tuple
import logging

logger = logging.getLogger(__name__)


# Known endpoint patterns for Russian bookmakers
BOOKMAKER_ENDPOINTS = {
    "winline": {
        "base_url": "https://winline.ru",
        "endpoints": [
            "https://winline.ru/api/content/line",
            "https://winline.ru/api/v1/line",
            "https://winline.ru/api/line",
            "https://winline.ru/api/content/events",
            "https://winline.ru/api/v1/events",
            "https://winline.ru/api/events",
            "https://winline.ru/api/content/live",
            "https://winline.ru/api/v1/live",
            "https://winline.ru/api/live",
            "https://winline.ru/api/content/football",
            "https://winline.ru/api/v1/football",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://winline.ru/",
            "Origin": "https://winline.ru",
        }
    },
    "fonbet": {
        "base_url": "https://fonbet.ru",
        "endpoints": [
            "https://api.fonbet.ru/line",
            "https://api.fonbet.ru/live",
            "https://api.fonbet.ru/feed",
            "https://api.fonbet.ru/lineFeed",
            "https://api.fonbet.ru/liveFeed",
            "https://api.fonbet.ru/api/v1/line",
            "https://api.fonbet.ru/api/v1/live",
            "https://api.fonbet.ru/api/v1/feed",
            "https://api.fonbet.ru/api/v1/events",
            "https://fonbet.ru/api/v1/line",
            "https://fonbet.ru/api/v1/live",
            "https://fonbet.ru/api/v1/feed",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://fonbet.ru/",
            "Origin": "https://fonbet.ru",
        }
    },
    "pari": {
        "base_url": "https://pari.ru",
        "endpoints": [
            "https://pari.ru/api/v1/line",
            "https://pari.ru/api/v1/live",
            "https://pari.ru/api/v1/feed",
            "https://pari.ru/api/v1/events",
            "https://pari.ru/api/line",
            "https://pari.ru/api/live",
            "https://pari.ru/api/feed",
            "https://pari.ru/api/events",
            "https://pari.ru/api/content/line",
            "https://pari.ru/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://pari.ru/",
            "Origin": "https://pari.ru",
        }
    },
    "olimp": {
        "base_url": "https://olimp.bet",
        "endpoints": [
            "https://olimp.bet/api/v1/line",
            "https://olimp.bet/api/v1/live",
            "https://olimp.bet/api/v1/feed",
            "https://olimp.bet/api/v1/events",
            "https://olimp.bet/api/line",
            "https://olimp.bet/api/live",
            "https://olimp.bet/api/feed",
            "https://olimp.bet/api/events",
            "https://olimp.bet/api/content/line",
            "https://olimp.bet/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://olimp.bet/",
            "Origin": "https://olimp.bet",
        }
    },
    "betboom": {
        "base_url": "https://betboom.ru",
        "endpoints": [
            "https://betboom.ru/api/v1/line",
            "https://betboom.ru/api/v1/live",
            "https://betboom.ru/api/v1/feed",
            "https://betboom.ru/api/v1/events",
            "https://betboom.ru/api/line",
            "https://betboom.ru/api/live",
            "https://betboom.ru/api/feed",
            "https://betboom.ru/api/events",
            "https://betboom.ru/api/content/line",
            "https://betboom.ru/api/content/live",
            "https://betboom.ru/api/v3/sports/football/live",
            "https://betboom.ru/api/v3/sports/football/prematch",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://betboom.ru/",
            "Origin": "https://betboom.ru",
        }
    },
    "1xstavka": {
        "base_url": "https://1xstavka.ru",
        "endpoints": [
            "https://1xstavka.ru/liveFeed",
            "https://1xstavka.ru/lineFeed",
            "https://1xstavka.ru/live",
            "https://1xstavka.ru/line",
            "https://1xstavka.ru/api/v1/line",
            "https://1xstavka.ru/api/v1/live",
            "https://1xstavka.ru/api/v1/feed",
            "https://1xstavka.ru/api/v1/events",
            "https://1xstavka.ru/api/line",
            "https://1xstavka.ru/api/live",
            "https://1xstavka.ru/api/feed",
            "https://1xstavka.ru/api/events",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://1xstavka.ru/",
            "Origin": "https://1xstavka.ru",
        }
    },
    "leon": {
        "base_url": "https://leon.ru",
        "endpoints": [
            "https://leon.ru/api/v1/line",
            "https://leon.ru/api/v1/live",
            "https://leon.ru/api/v1/feed",
            "https://leon.ru/api/v1/events",
            "https://leon.ru/api/line",
            "https://leon.ru/api/live",
            "https://leon.ru/api/feed",
            "https://leon.ru/api/events",
            "https://leon.ru/api/content/line",
            "https://leon.ru/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://leon.ru/",
            "Origin": "https://leon.ru",
        }
    },
    "marathon": {
        "base_url": "https://marathonbet.com",
        "endpoints": [
            "https://marathonbet.com/api/v1/line",
            "https://marathonbet.com/api/v1/live",
            "https://marathonbet.com/api/v1/feed",
            "https://marathonbet.com/api/v1/events",
            "https://marathonbet.com/api/line",
            "https://marathonbet.com/api/live",
            "https://marathonbet.com/api/feed",
            "https://marathonbet.com/api/events",
            "https://marathonbet.com/api/content/line",
            "https://marathonbet.com/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://marathonbet.com/",
            "Origin": "https://marathonbet.com",
        }
    },
    "betcity": {
        "base_url": "https://betcity.ru",
        "endpoints": [
            "https://betcity.ru/api/v1/line",
            "https://betcity.ru/api/v1/live",
            "https://betcity.ru/api/v1/feed",
            "https://betcity.ru/api/v1/events",
            "https://betcity.ru/api/line",
            "https://betcity.ru/api/live",
            "https://betcity.ru/api/feed",
            "https://betcity.ru/api/events",
            "https://betcity.ru/api/content/line",
            "https://betcity.ru/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://betcity.ru/",
            "Origin": "https://betcity.ru",
        }
    },
    "pinup": {
        "base_url": "https://pin-up.ru",
        "endpoints": [
            "https://pin-up.ru/api/v1/line",
            "https://pin-up.ru/api/v1/live",
            "https://pin-up.ru/api/v1/feed",
            "https://pin-up.ru/api/v1/events",
            "https://pin-up.ru/api/line",
            "https://pin-up.ru/api/live",
            "https://pin-up.ru/api/feed",
            "https://pin-up.ru/api/events",
            "https://pin-up.ru/api/content/line",
            "https://pin-up.ru/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://pin-up.ru/",
            "Origin": "https://pin-up.ru",
        }
    },
    "zenit": {
        "base_url": "https://zenit.bet",
        "endpoints": [
            "https://zenit.bet/api/v1/line",
            "https://zenit.bet/api/v1/live",
            "https://zenit.bet/api/v1/feed",
            "https://zenit.bet/api/v1/events",
            "https://zenit.bet/api/line",
            "https://zenit.bet/api/live",
            "https://zenit.bet/api/feed",
            "https://zenit.bet/api/events",
            "https://zenit.bet/api/content/line",
            "https://zenit.bet/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://zenit.bet/",
            "Origin": "https://zenit.bet",
        }
    },
    "olimpbet": {
        "base_url": "https://olimpbet.kz",
        "endpoints": [
            "https://olimpbet.kz/api/v1/line",
            "https://olimpbet.kz/api/v1/live",
            "https://olimpbet.kz/api/v1/feed",
            "https://olimpbet.kz/api/v1/events",
            "https://olimpbet.kz/api/line",
            "https://olimpbet.kz/api/live",
            "https://olimpbet.kz/api/feed",
            "https://olimpbet.kz/api/events",
            "https://olimpbet.kz/api/content/line",
            "https://olimpbet.kz/api/content/live",
        ],
        "headers": {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://olimpbet.kz/",
            "Origin": "https://olimpbet.kz",
        }
    }
}


class DiscoveryService:
    """Discovers working API endpoints for bookmakers"""
    
    def __init__(self):
        self.results: Dict[str, List[Dict]] = {}
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def get_session(self) -> aiohttp.ClientSession:
        if self.session is None or self.session.closed:
            timeout = aiohttp.ClientTimeout(total=10)
            connector = aiohttp.TCPConnector(
                limit=10,
                limit_per_host=5,
                ttl_dns_cache=300
            )
            self.session = aiohttp.ClientSession(
                timeout=timeout,
                connector=connector
            )
        return self.session
    
    async def close(self):
        if self.session and not self.session.closed:
            await self.session.close()
    
    async def test_endpoint(self, url: str, headers: Dict) -> Tuple[bool, Optional[Dict], int]:
        """Test a single endpoint"""
        try:
            session = await self.get_session()
            async with session.get(url, headers=headers) as resp:
                if resp.status == 200:
                    try:
                        data = await resp.json()
                        return True, data, len(str(data))
                    except:
                        text = await resp.text()
                        return True, {"text_preview": text[:500]}, len(text)
                return False, None, resp.status
        except Exception as e:
            return False, {"error": str(e)}, 0
    
    async def discover_bookmaker(self, bk_slug: str) -> List[Dict]:
        """Discover working endpoints for a bookmaker"""
        if bk_slug not in BOOKMAKER_ENDPOINTS:
            return [{"error": f"Unknown bookmaker: {bk_slug}"}]
        
        bk_config = BOOKMAKER_ENDPOINTS[bk_slug]
        results = []
        
        logger.info(f"🔍 Discovering endpoints for {bk_slug}...")
        
        for url in bk_config["endpoints"]:
            success, data, size = await self.test_endpoint(url, bk_config["headers"])
            
            result = {
                "url": url,
                "success": success,
                "response_size": size,
                "has_data": bool(data),
                "preview": str(data)[:200] if data else None,
            }
            results.append(result)
            
            status = "✅" if success else "❌"
            logger.info(f"  {status} {url} ({size} bytes)")
            
            await asyncio.sleep(0.5)  # Rate limit
        
        working = [r for r in results if r["success"]]
        logger.info(f"📊 {bk_slug}: {len(working)}/{len(results)} endpoints working")
        
        return results
    
    async def discover_all(self) -> Dict[str, List[Dict]]:
        """Discover endpoints for all bookmakers"""
        for bk_slug in BOOKMAKER_ENDPOINTS:
            self.results[bk_slug] = await self.discover_bookmaker(bk_slug)
        
        return self.results
    
    def get_summary(self) -> Dict:
        """Get discovery summary"""
        summary = {}
        for bk_slug, results in self.results.items():
            working = [r for r in results if r.get("success")]
            summary[bk_slug] = {
                "total_endpoints": len(results),
                "working_endpoints": len(working),
                "working_urls": [r["url"] for r in working],
            }
        return summary
    
    def save_results(self, filepath: str = "discovery_results.json"):
        """Save results to JSON file"""
        import json
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump({
                "summary": self.get_summary(),
                "details": self.results
            }, f, indent=2, ensure_ascii=False)
        logger.info(f"💾 Results saved to {filepath}")


async def run_discovery():
    """Run full discovery"""
    service = DiscoveryService()
    try:
        results = await service.discover_all()
        service.save_results()
        
        print("\n" + "="*60)
        print("🔍 DISCOVERY SUMMARY")
        print("="*60)
        
        summary = service.get_summary()
        for bk_slug, stats in summary.items():
            working = stats["working_endpoints"]
            total = stats["total_endpoints"]
            status = "✅" if working > 0 else "❌"
            print(f"{status} {bk_slug}: {working}/{total} endpoints working")
            if working > 0:
                for url in stats["working_urls"][:3]:
                    print(f"   → {url}")
        
        print("="*60)
        
    finally:
        await service.close()


if __name__ == "__main__":
    asyncio.run(run_discovery())
