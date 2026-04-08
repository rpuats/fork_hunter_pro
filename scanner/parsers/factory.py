# scanner/parsers/factory.py
"""
Parser factory pattern implementation.
Creates parsers by slug, supports both mock and real parsers,
provides health checks and stats.
"""
import os
import logging
from typing import Dict, Type, List, Optional, Any

from scanner.parsers.base import BaseParser
from scanner.parsers.circuit_breaker import CircuitBreaker

logger = logging.getLogger(__name__)


class ParserFactory:
    """
    Factory for creating and managing bookmaker parsers.

    Features:
    - Creates parsers by slug
    - Supports both mock and real parsers
    - Auto-registers parsers via decorator
    - Provides health check and stats for each parser
    - Integrates circuit breaker pattern
    """

    _registry: Dict[str, Type[BaseParser]] = {}
    _mock_registry: Dict[str, Type[BaseParser]] = {}
    _circuit_breakers: Dict[str, CircuitBreaker] = {}

    @classmethod
    def register(cls, parser_cls: Type[BaseParser], is_mock: bool = False):
        """Register a parser class in the factory."""
        registry = cls._mock_registry if is_mock else cls._registry
        registry[parser_cls.slug] = parser_cls
        logger.debug(f"Registered parser '{parser_cls.slug}' ({parser_cls.name})")
        return parser_cls

    @classmethod
    def create(
        cls,
        slug: str,
        use_mock: bool = False,
        circuit_breaker: Optional[CircuitBreaker] = None,
        **kwargs,
    ) -> Optional[BaseParser]:
        """
        Create a parser instance by slug.

        Args:
            slug: Parser slug identifier
            use_mock: If True, use mock parser if available
            circuit_breaker: Optional circuit breaker for resilience
            **kwargs: Additional arguments passed to parser constructor

        Returns:
            Parser instance or None if not found
        """
        if use_mock:
            parser_cls = cls._mock_registry.get(slug)
            if parser_cls is None:
                parser_cls = cls._registry.get(slug)
        else:
            parser_cls = cls._registry.get(slug)

        if parser_cls is None:
            logger.warning(f"Parser not found for slug: '{slug}'")
            return None

        try:
            parser = parser_cls(**kwargs)
            if circuit_breaker:
                cls._circuit_breakers[slug] = circuit_breaker
            logger.info(f"Created parser: {parser.name} ({parser.slug})")
            return parser
        except Exception as e:
            logger.error(f"Failed to create parser '{slug}': {e}")
            return None

    @classmethod
    def create_all(
        cls,
        slugs: Optional[List[str]] = None,
        use_mock: bool = False,
        **kwargs,
    ) -> List[BaseParser]:
        """
        Create multiple parser instances.

        Args:
            slugs: List of parser slugs. If None, creates all registered parsers.
            use_mock: If True, use mock parsers.
            **kwargs: Additional arguments passed to parser constructors.

        Returns:
            List of parser instances.
        """
        if slugs is None:
            slugs = list(cls._mock_registry.keys() if use_mock else cls._registry.keys())

        parsers = []
        for slug in slugs:
            parser = cls.create(slug, use_mock=use_mock, **kwargs)
            if parser:
                parsers.append(parser)

        logger.info(f"Created {len(parsers)} parsers")
        return parsers

    @classmethod
    def get_available_slugs(cls, include_mock: bool = False) -> List[str]:
        """Get list of available parser slugs."""
        slugs = list(cls._registry.keys())
        if include_mock:
            slugs.extend(cls._mock_registry.keys())
        return sorted(set(slugs))

    @classmethod
    def has_parser(cls, slug: str) -> bool:
        """Check if a parser is registered."""
        return slug in cls._registry or slug in cls._mock_registry

    @classmethod
    def get_parser_info(cls, slug: str) -> Optional[Dict[str, Any]]:
        """Get information about a registered parser."""
        parser_cls = cls._registry.get(slug) or cls._mock_registry.get(slug)
        if parser_cls is None:
            return None

        return {
            "slug": parser_cls.slug,
            "name": parser_cls.name,
            "base_url": getattr(parser_cls, "base_url", ""),
            "is_mock": parser_cls.__module__.endswith("mock_parser"),
        }

    @classmethod
    def get_all_parser_info(cls) -> List[Dict[str, Any]]:
        """Get information about all registered parsers."""
        info = []
        all_slugs = set(cls._registry.keys()) | set(cls._mock_registry.keys())
        for slug in sorted(all_slugs):
            parser_info = cls.get_parser_info(slug)
            if parser_info:
                info.append(parser_info)
        return info

    @classmethod
    def get_circuit_breaker(cls, slug: str) -> Optional[CircuitBreaker]:
        """Get circuit breaker for a parser."""
        return cls._circuit_breakers.get(slug)

    @classmethod
    def get_stats(cls) -> Dict[str, Any]:
        """Get stats for all parsers and circuit breakers."""
        stats = {
            "total_registered": len(cls._registry),
            "total_mock": len(cls._mock_registry),
            "parsers": {},
            "circuit_breakers": {},
        }

        for slug, parser_cls in cls._registry.items():
            stats["parsers"][slug] = {
                "name": parser_cls.name,
                "type": "real",
                "has_mock": slug in cls._mock_registry,
            }

        for slug, parser_cls in cls._mock_registry.items():
            if slug not in stats["parsers"]:
                stats["parsers"][slug] = {
                    "name": parser_cls.name,
                    "type": "mock",
                }
            else:
                stats["parsers"][slug]["mock_name"] = parser_cls.name

        for slug, cb in cls._circuit_breakers.items():
            stats["circuit_breakers"][slug] = cb.get_status()

        return stats

    @classmethod
    def reset(cls):
        """Reset factory state (useful for testing)."""
        cls._registry.clear()
        cls._mock_registry.clear()
        cls._circuit_breakers.clear()


def auto_register_parsers():
    """Auto-register all real and mock parsers."""
    from scanner.parsers.winline_parser import WinlineParser
    from scanner.parsers.winline_playwright import WinlinePlaywrightParser
    from scanner.parsers.pari_playwright import PariPlaywrightParser
    from scanner.parsers.fonbet_playwright import FonbetPlaywrightParser
    from scanner.parsers.olimp_parser import OlimpParser
    from scanner.parsers.olimpbet_parser import OlimpBetParser
    from scanner.parsers.pari_parser import PariParser
    from scanner.parsers.pari_api import PariParser as PariApiParser
    from scanner.parsers.marathon_parser import MarathonParser
    from scanner.parsers.marathon_playwright import MarathonPlaywrightParser
    from scanner.parsers.betboom_parser import BetBoomParser
    from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser
    from scanner.parsers.fonbet_parser import FonbetParser
    from scanner.parsers.fonbet_api import FonbetParser as FonbetApiParser
    from scanner.parsers.bettery_api import BetteryParser
    from scanner.parsers.onexstavka_parser import OnexStavkaParser
    from scanner.parsers.leon_parser import LeonParser
    from scanner.parsers.leon_api import LeonParser as LeonApiParser
    from scanner.parsers.betcity_parser import BetcityParser
    from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
    from scanner.parsers.pinup_parser import PinupParser
    from scanner.parsers.zenit_parser import ZenitParser
    from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
    from scanner.parsers.ligastavok_playwright import LigaStavokPlaywrightParser
    from scanner.parsers.olimp_parser import OlimpParser as OlimpApiParser
    from scanner.parsers._24bet_playwright import _24betPlaywrightParser
    from scanner.parsers.sportbet_playwright import SportbetPlaywrightParser
    from scanner.parsers.marathon_api import MarathonApiParser
    from scanner.parsers._24bet_api import _24betApiParser
    from scanner.parsers.sportbet_api import SportbetApiParser

    ParserFactory.register(WinlineParser)
    ParserFactory.register(WinlinePlaywrightParser)
    ParserFactory.register(PariPlaywrightParser)
    ParserFactory.register(FonbetPlaywrightParser)
    ParserFactory.register(OlimpParser)
    ParserFactory.register(OlimpBetParser)
    ParserFactory.register(PariParser)
    ParserFactory.register(PariApiParser)
    ParserFactory.register(MarathonParser)
    ParserFactory.register(MarathonPlaywrightParser)
    ParserFactory.register(BetBoomParser)
    ParserFactory.register(BetBoomPlaywrightParser)
    ParserFactory.register(FonbetParser)
    ParserFactory.register(FonbetApiParser)
    ParserFactory.register(BetteryParser)
    ParserFactory.register(OnexStavkaParser)
    ParserFactory.register(LeonParser)
    ParserFactory.register(LeonApiParser)
    ParserFactory.register(BetcityParser)
    ParserFactory.register(BetcityPlaywrightParser)
    ParserFactory.register(PinupParser)
    ParserFactory.register(ZenitParser)
    ParserFactory.register(ZenitPlaywrightParser)
    ParserFactory.register(LigaStavokPlaywrightParser)
    ParserFactory.register(OlimpApiParser)
    ParserFactory.register(_24betPlaywrightParser)
    ParserFactory.register(SportbetPlaywrightParser)
    ParserFactory.register(SportbetApiParser)
    ParserFactory.register(MarathonApiParser)
    ParserFactory.register(_24betApiParser)

    from scanner.parsers.mock_parser import (
        MockWinlineParser, MockFonbetParser, MockPariParser,
        MockOlimpParser, MockBetBoomParser, Mock1xStavkaParser,
        MockLeonParser, MockMarathonParser, MockBetcityParser,
        MockPinupParser, MockZenitParser, MockOlimpbetParser,
    )

    ParserFactory.register(MockWinlineParser, is_mock=True)
    ParserFactory.register(MockFonbetParser, is_mock=True)
    ParserFactory.register(MockPariParser, is_mock=True)
    ParserFactory.register(MockOlimpParser, is_mock=True)
    ParserFactory.register(MockBetBoomParser, is_mock=True)
    ParserFactory.register(Mock1xStavkaParser, is_mock=True)
    ParserFactory.register(MockLeonParser, is_mock=True)
    ParserFactory.register(MockMarathonParser, is_mock=True)
    ParserFactory.register(MockBetcityParser, is_mock=True)
    ParserFactory.register(MockPinupParser, is_mock=True)
    ParserFactory.register(MockZenitParser, is_mock=True)
    ParserFactory.register(MockOlimpbetParser, is_mock=True)

    logger.info("All parsers auto-registered in factory")
