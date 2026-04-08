# core/proxy_manager.py
"""
Proxy Manager - Rotating proxy pool for scraping
"""
import asyncio
import random
import time
import logging
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass
from collections import defaultdict
import aiohttp

logger = logging.getLogger(__name__)


@dataclass
class Proxy:
    """Proxy configuration"""
    url: str
    username: Optional[str] = None
    password: Optional[str] = None
    protocol: str = 'http'
    latency_ms: float = 0.0
    success_count: int = 0
    fail_count: int = 0
    last_used: float = 0.0
    is_banned: bool = False
    ban_until: float = 0.0
    
    @property
    def success_rate(self) -> float:
        total = self.success_count + self.fail_count
        return self.success_count / total if total > 0 else 1.0
    
    @property
    def is_healthy(self) -> bool:
        if self.is_banned and time.time() < self.ban_until:
            return False
        if self.fail_count > 10 and self.success_rate < 0.5:
            return False
        return True


class ProxyManager:
    """
    Manages proxy rotation for scraping.
    Features:
    - Automatic proxy testing
    - Failure tracking
    - Ban rotation
    - Geographic distribution
    - Protocol support (HTTP, SOCKS5)
    """
    
    def __init__(self):
        self.proxies: List[Proxy] = []
        self.proxy_stats: Dict[str, Dict] = defaultdict(lambda: {
            'successes': 0, 'failures': 0, 'bans': 0
        })
        self.current_index = 0
        self._lock = asyncio.Lock()
    
    def add_proxy(self, proxy: Proxy):
        """Add a proxy to the pool"""
        self.proxies.append(proxy)
        logger.info(f"Added proxy: {proxy.url}")
    
    def add_proxies_from_config(self, proxy_list: List[Dict]):
        """Add proxies from config format"""
        for p in proxy_list:
            proxy = Proxy(
                url=p.get('url', ''),
                username=p.get('username'),
                password=p.get('password'),
                protocol=p.get('protocol', 'http')
            )
            self.add_proxy(proxy)
    
    async def get_proxy(self, bookmaker: Optional[str] = None) -> Optional[Dict]:
        """Get next available proxy"""
        async with self._lock:
            healthy = [p for p in self.proxies if p.is_healthy]
            
            if not healthy:
                logger.warning("No healthy proxies available!")
                return None
            
            # Strategy: rotate through proxies
            proxy = healthy[self.current_index % len(healthy)]
            self.current_index += 1
            
            proxy.last_used = time.time()
            
            return {
                'http': proxy.url,
                'https': proxy.url,
            }
    
    async def report_success(self, proxy_url: str, latency_ms: float = 0):
        """Report successful proxy usage"""
        async with self._lock:
            for proxy in self.proxies:
                if proxy.url == proxy_url:
                    proxy.success_count += 1
                    proxy.latency_ms = (proxy.latency_ms * 0.7 + latency_ms * 0.3)
                    self.proxy_stats[proxy_url]['successes'] += 1
                    break
    
    async def report_failure(self, proxy_url: str, error_type: str = 'timeout'):
        """Report proxy failure"""
        async with self._lock:
            for proxy in self.proxies:
                if proxy.url == proxy_url:
                    proxy.fail_count += 1
                    self.proxy_stats[proxy_url]['failures'] += 1
                    
                    # Ban proxy if too many failures
                    if proxy.fail_count > 5 and proxy.success_rate < 0.3:
                        proxy.is_banned = True
                        proxy.ban_until = time.time() + 300  # 5 min ban
                        self.proxy_stats[proxy_url]['bans'] += 1
                        logger.warning(f"Proxy banned: {proxy_url}")
                    break
    
    async def test_proxies(self) -> Dict[str, bool]:
        """Test all proxies and return status"""
        results = {}
        
        for proxy in self.proxies:
            try:
                start = time.time()
                
                async with aiohttp.ClientSession() as session:
                    timeout = aiohttp.ClientTimeout(total=10)
                    
                    proxy_auth = None
                    if proxy.username and proxy.password:
                        import aiohttp_socks
                        proxy_auth = aiohttp_socks.ProxyAuth(
                            proxy.username, proxy.password
                        )
                    
                    async with session.get(
                        'https://httpbin.org/ip',
                        proxy=proxy.url,
                        timeout=timeout
                    ) as resp:
                        latency = (time.time() - start) * 1000
                        
                        if resp.status == 200:
                            proxy.latency_ms = latency
                            proxy.success_count += 1
                            results[proxy.url] = True
                        else:
                            proxy.fail_count += 1
                            results[proxy.url] = False
                            
            except Exception as e:
                proxy.fail_count += 1
                results[proxy.url] = False
                logger.error(f"Proxy test failed {proxy.url}: {e}")
        
        return results
    
    def get_stats(self) -> Dict:
        """Get proxy pool statistics"""
        total = len(self.proxies)
        healthy = sum(1 for p in self.proxies if p.is_healthy)
        
        avg_latency = sum(p.latency_ms for p in self.proxies) / total if total > 0 else 0
        avg_success = sum(p.success_rate for p in self.proxies) / total if total > 0 else 0
        
        return {
            'total_proxies': total,
            'healthy_proxies': healthy,
            'banned_proxies': total - healthy,
            'avg_latency_ms': round(avg_latency, 1),
            'avg_success_rate': round(avg_success * 100, 1),
            'total_requests': sum(p.success_count + p.fail_count for p in self.proxies),
        }
    
    def rotate_geo(self, target_country: str = 'RU') -> Optional[Dict]:
        """Get proxy from specific geographic region"""
        # For now, just return random healthy proxy
        healthy = [p for p in self.proxies if p.is_healthy]
        if not healthy:
            return None
        
        proxy = random.choice(healthy)
        return {
            'http': proxy.url,
            'https': proxy.url,
        }


# Default free proxies (for testing - replace with paid proxies in production)
DEFAULT_PROXIES = [
    {'url': 'http://proxy1.example.com:8080'},
    {'url': 'http://proxy2.example.com:8080'},
    {'url': 'http://proxy3.example.com:8080'},
]


# Global instance
proxy_manager = ProxyManager()

# Add default proxies (in production, use paid proxies from providers like Luminati, SmartProxy, etc.)
# proxy_manager.add_proxies_from_config(DEFAULT_PROXIES)
