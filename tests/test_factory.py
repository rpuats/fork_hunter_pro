# tests/test_factory.py
import pytest
import sys
import os
from unittest.mock import MagicMock, patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from scanner.parsers.factory import ParserFactory
from scanner.parsers.circuit_breaker import CircuitBreaker


class DummyParser:
    name = "Dummy"
    slug = "dummy"
    base_url = "https://dummy.example"

    def __init__(self, **kwargs):
        pass


class DummyParserV2:
    name = "DummyV2"
    slug = "dummy_v2"
    base_url = "https://dummy2.example"

    def __init__(self, **kwargs):
        pass


class MockDummyParser:
    name = "MockDummy"
    slug = "dummy"
    base_url = "https://mock.dummy.example"

    def __init__(self, **kwargs):
        pass


class TestParserFactoryRegistration:
    def setup_method(self):
        ParserFactory.reset()

    def test_register_parser(self):
        ParserFactory.register(DummyParser)
        assert ParserFactory.has_parser("dummy")

    def test_register_mock_parser(self):
        ParserFactory.register(MockDummyParser, is_mock=True)
        assert ParserFactory.has_parser("dummy")
        assert "dummy" in ParserFactory._mock_registry

    def test_register_multiple_parsers(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(DummyParserV2)
        assert ParserFactory.has_parser("dummy")
        assert ParserFactory.has_parser("dummy_v2")

    def test_register_returns_class(self):
        result = ParserFactory.register(DummyParser)
        assert result is DummyParser


class TestParserFactoryCreate:
    def setup_method(self):
        ParserFactory.reset()

    def test_create_existing_parser(self):
        ParserFactory.register(DummyParser)
        parser = ParserFactory.create("dummy")
        assert parser is not None
        assert parser.slug == "dummy"

    def test_create_missing_parser_returns_none(self):
        parser = ParserFactory.create("nonexistent")
        assert parser is None

    def test_create_with_use_mock_prefers_mock(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(MockDummyParser, is_mock=True)
        parser = ParserFactory.create("dummy", use_mock=True)
        assert parser is not None
        assert parser.name == "MockDummy"

    def test_create_with_use_mock_falls_back_to_real(self):
        ParserFactory.register(DummyParser)
        parser = ParserFactory.create("dummy", use_mock=True)
        assert parser is not None
        assert parser.name == "Dummy"

    def test_create_with_circuit_breaker(self):
        ParserFactory.register(DummyParser)
        cb = CircuitBreaker(slug="dummy")
        parser = ParserFactory.create("dummy", circuit_breaker=cb)
        assert parser is not None
        assert ParserFactory.get_circuit_breaker("dummy") is cb

    def test_create_passes_kwargs(self):
        class KwargParser:
            name = "Kwarg"
            slug = "kwarg"

            def __init__(self, custom_arg=None):
                self.custom_arg = custom_arg

        ParserFactory.register(KwargParser)
        parser = ParserFactory.create("kwarg", custom_arg="hello")
        assert parser.custom_arg == "hello"

    def test_create_with_exception_returns_none(self, caplog):
        class BrokenParser:
            name = "Broken"
            slug = "broken"

            def __init__(self, **kwargs):
                raise ValueError("boom")

        ParserFactory.register(BrokenParser)
        parser = ParserFactory.create("broken")
        assert parser is None


class TestParserFactoryCreateAll:
    def setup_method(self):
        ParserFactory.reset()

    def test_create_all_registers_all(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(DummyParserV2)
        parsers = ParserFactory.create_all()
        assert len(parsers) == 2

    def test_create_all_with_specific_slugs(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(DummyParserV2)
        parsers = ParserFactory.create_all(slugs=["dummy"])
        assert len(parsers) == 1
        assert parsers[0].slug == "dummy"

    def test_create_all_skips_missing(self):
        ParserFactory.register(DummyParser)
        parsers = ParserFactory.create_all(slugs=["dummy", "nonexistent"])
        assert len(parsers) == 1


class TestParserFactoryQueries:
    def setup_method(self):
        ParserFactory.reset()

    def test_get_available_slugs(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(DummyParserV2)
        slugs = ParserFactory.get_available_slugs()
        assert "dummy" in slugs
        assert "dummy_v2" in slugs

    def test_get_available_slugs_with_mock(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(MockDummyParser, is_mock=True)
        slugs = ParserFactory.get_available_slugs(include_mock=True)
        assert "dummy" in slugs

    def test_get_available_slugs_sorted(self):
        ParserFactory.register(DummyParserV2)
        ParserFactory.register(DummyParser)
        slugs = ParserFactory.get_available_slugs()
        assert slugs == sorted(slugs)

    def test_has_parser_true(self):
        ParserFactory.register(DummyParser)
        assert ParserFactory.has_parser("dummy")

    def test_has_parser_false(self):
        assert ParserFactory.has_parser("nonexistent") is False

    def test_get_parser_info(self):
        ParserFactory.register(DummyParser)
        info = ParserFactory.get_parser_info("dummy")
        assert info is not None
        assert info["slug"] == "dummy"
        assert info["name"] == "Dummy"
        assert info["base_url"] == "https://dummy.example"

    def test_get_parser_info_missing(self):
        assert ParserFactory.get_parser_info("nonexistent") is None

    def test_get_all_parser_info(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(DummyParserV2)
        info = ParserFactory.get_all_parser_info()
        assert len(info) == 2

    def test_get_stats(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(MockDummyParser, is_mock=True)
        cb = CircuitBreaker(slug="dummy")
        ParserFactory.create("dummy", circuit_breaker=cb)
        stats = ParserFactory.get_stats()
        assert stats["total_registered"] == 1
        assert stats["total_mock"] == 1
        assert "dummy" in stats["parsers"]
        assert "dummy" in stats["circuit_breakers"]

    def test_reset(self):
        ParserFactory.register(DummyParser)
        ParserFactory.register(MockDummyParser, is_mock=True)
        cb = CircuitBreaker(slug="dummy")
        ParserFactory.create("dummy", circuit_breaker=cb)
        ParserFactory.reset()
        assert ParserFactory.has_parser("dummy") is False
        assert len(ParserFactory._registry) == 0
        assert len(ParserFactory._mock_registry) == 0
        assert len(ParserFactory._circuit_breakers) == 0
