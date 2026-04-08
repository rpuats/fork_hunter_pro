# tests/test_event_pool.py
import pytest
import sys
import os
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.event_pool import EventPool, EventEntry, BloomFilter


def make_event(home, away, bk, home_odds=2.0, away_odds=2.0, draw_odds=None):
    return {
        'home_team': home,
        'away_team': away,
        'bookmaker': bk,
        'home_odds': home_odds,
        'away_odds': away_odds,
        'draw_odds': draw_odds,
        'sport': 'football',
        'market': '1x2',
    }


class TestEventEntry:
    def test_create_entry(self):
        entry = EventEntry(
            key="test",
            hash_value=123,
            data={'foo': 'bar'},
            timestamp=time.time(),
        )
        assert entry.key == "test"
        assert entry.hash_value == 123
        assert entry.version == 0

    def test_update_with_change(self):
        entry = EventEntry(
            key="test",
            hash_value=hash("2.02.00"),
            data=make_event("A", "B", "bk1", 2.0, 2.0),
            timestamp=time.time(),
        )
        new_data = make_event("A", "B", "bk1", 2.5, 2.0)
        changed = entry.update(new_data)
        assert changed is True
        assert entry.version == 1

    def test_update_without_change(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1", 2.0, 2.0)
        pool.upsert(event)
        event2 = make_event("A", "B", "bk1", 2.0, 2.0)
        inserted, changed = pool.upsert(event2)
        assert inserted is False
        assert changed is False
        stored = pool.get("a|b|bk1|1x2")
        assert stored['home_team'] == 'A'


class TestBloomFilter:
    def test_add_and_check(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        bf.add("item1")
        assert bf.might_contain("item1") is True

    def test_might_contain_never_added(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        assert bf.might_contain("item1") is False

    def test_clear(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        bf.add("item1")
        assert bf.might_contain("item1") is True
        bf.clear()
        assert bf.might_contain("item1") is False

    def test_multiple_items(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        for i in range(100):
            bf.add(f"item_{i}")
        for i in range(100):
            assert bf.might_contain(f"item_{i}") is True

    def test_false_positive_rate(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        items = [f"item_{i}" for i in range(100)]
        for item in items:
            bf.add(item)
        false_positives = 0
        total_tests = 1000
        for i in range(total_tests):
            if bf.might_contain(f"nonexistent_{i}"):
                false_positives += 1
        rate = false_positives / total_tests
        assert rate < 0.1

    def test_size_and_hash_count(self):
        bf = BloomFilter(capacity=1000, error_rate=0.01)
        assert bf._size > 0
        assert bf._hash_count > 0


class TestEventPoolInit:
    def test_default_values(self):
        pool = EventPool()
        assert pool.max_size == 50000
        assert pool.eviction_threshold == 0.9
        assert pool.stale_ttl == 300.0

    def test_custom_values(self):
        pool = EventPool(max_size=100, eviction_threshold=0.8, stale_ttl=60.0)
        assert pool.max_size == 100
        assert pool.eviction_threshold == 0.8
        assert pool.stale_ttl == 60.0


class TestEventPoolUpsert:
    def test_insert_new_event(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1", 2.0, 2.0)
        inserted, changed = pool.upsert(event)
        assert inserted is True
        assert changed is True
        assert pool.get_count() == 1

    def test_update_existing_event(self):
        pool = EventPool()
        event1 = make_event("A", "B", "bk1", 2.0, 2.0)
        pool.upsert(event1)
        event2 = make_event("A", "B", "bk1", 2.5, 2.0)
        inserted, changed = pool.upsert(event2)
        assert inserted is False
        assert changed is True
        assert pool.get_count() == 1

    def test_update_no_change(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1", 2.0, 2.0)
        pool.upsert(event)
        inserted, changed = pool.upsert(event)
        assert inserted is False
        assert changed is False

    def test_insert_empty_key(self):
        pool = EventPool()
        event = {'home_team': '', 'away_team': '', 'bookmaker': 'bk1'}
        inserted, changed = pool.upsert(event)
        assert inserted is False
        assert changed is False

    def test_insert_missing_bookmaker(self):
        pool = EventPool()
        event = {'home_team': 'A', 'away_team': 'B'}
        inserted, changed = pool.upsert(event)
        assert inserted is False
        assert changed is False


class TestEventPoolBatchUpsert:
    def test_batch_insert(self):
        pool = EventPool()
        events = [
            make_event("A", "B", "bk1", 2.0, 2.0),
            make_event("C", "D", "bk2", 2.0, 2.0),
        ]
        inserted, changed = pool.upsert_batch(events)
        assert inserted == 2
        assert changed == 0
        assert pool.get_count() == 2

    def test_batch_update(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1", 2.0, 2.0))
        events = [make_event("A", "B", "bk1", 2.5, 2.0)]
        inserted, changed = pool.upsert_batch(events)
        assert inserted == 0
        assert changed == 1

    def test_batch_mixed(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1", 2.0, 2.0))
        events = [
            make_event("A", "B", "bk1", 2.5, 2.0),
            make_event("C", "D", "bk2", 2.0, 2.0),
        ]
        inserted, changed = pool.upsert_batch(events)
        assert inserted == 1
        assert changed == 1

    def test_batch_with_empty_key(self):
        pool = EventPool()
        events = [
            make_event("A", "B", "bk1"),
            {'home_team': '', 'away_team': ''},
        ]
        inserted, changed = pool.upsert_batch(events)
        assert inserted == 1


class TestEventPoolGetChanged:
    def test_first_run_returns_all(self):
        pool = EventPool()
        events = [make_event("A", "B", "bk1", 2.0, 2.0)]
        changed = pool.get_changed(events)
        assert len(changed) == 1

    def test_no_changes_returns_empty(self):
        pool = EventPool()
        events = [make_event("A", "B", "bk1", 2.0, 2.0)]
        pool.upsert_batch(events)
        changed = pool.get_changed(events)
        assert len(changed) == 0

    def test_detects_odds_change(self):
        pool = EventPool()
        events1 = [make_event("A", "B", "bk1", 2.0, 2.0)]
        pool.get_changed(events1)
        events2 = [make_event("A", "B", "bk1", 2.5, 2.0)]
        changed = pool.get_changed(events2)
        assert len(changed) == 1

    def test_removes_stale_events(self):
        pool = EventPool()
        events1 = [
            make_event("A", "B", "bk1", 2.0, 2.0),
            make_event("C", "D", "bk2", 2.0, 2.0),
        ]
        pool.upsert_batch(events1)
        events2 = [make_event("A", "B", "bk1", 2.0, 2.0)]
        pool.get_changed(events2)
        assert pool.get_count() == 1


class TestEventPoolGet:
    def test_get_existing(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1", 2.0, 2.0)
        pool.upsert(event)
        result = pool.get("a|b|bk1|1x2")
        assert result is not None
        assert result['home_team'] == 'A'

    def test_get_missing(self):
        pool = EventPool()
        assert pool.get("nonexistent") is None

    def test_get_all(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1"))
        pool.upsert(make_event("C", "D", "bk2"))
        all_events = pool.get_all()
        assert len(all_events) == 2

    def test_get_count(self):
        pool = EventPool()
        assert pool.get_count() == 0
        pool.upsert(make_event("A", "B", "bk1"))
        assert pool.get_count() == 1


class TestEventPoolEviction:
    def test_eviction_on_full_pool(self):
        pool = EventPool(max_size=10, eviction_threshold=0.9)
        for i in range(15):
            pool.upsert(make_event(f"A{i}", f"B{i}", f"bk{i}"))
        assert pool.get_count() <= 10

    def test_stats_after_eviction(self):
        pool = EventPool(max_size=10, eviction_threshold=0.9)
        for i in range(15):
            pool.upsert(make_event(f"A{i}", f"B{i}", f"bk{i}"))
        stats = pool.stats()
        assert stats['evictions'] > 0


class TestEventPoolStaleCleanup:
    def test_cleanup_stale_events(self):
        pool = EventPool(stale_ttl=0.01)
        pool.upsert(make_event("A", "B", "bk1"))
        time.sleep(0.02)
        removed = pool.cleanup_stale()
        assert removed == 1
        assert pool.get_count() == 0

    def test_no_cleanup_for_fresh_events(self):
        pool = EventPool(stale_ttl=300.0)
        pool.upsert(make_event("A", "B", "bk1"))
        removed = pool.cleanup_stale()
        assert removed == 0


class TestEventPoolBloom:
    def test_might_have_existing(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1")
        pool.upsert(event)
        assert pool.might_have(event) is True

    def test_might_have_new(self):
        pool = EventPool()
        event = make_event("A", "B", "bk1")
        pool.upsert(event)
        new_event = make_event("X", "Y", "bk99")
        assert pool.might_have(new_event) is False

    def test_might_have_empty_key(self):
        pool = EventPool()
        event = {'home_team': '', 'away_team': ''}
        assert pool.might_have(event) is False

    def test_reset_bloom(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1"))
        pool.reset_bloom()
        assert pool.might_have(make_event("A", "B", "bk1")) is True


class TestEventPoolStats:
    def test_stats_initial(self):
        pool = EventPool()
        stats = pool.stats()
        assert stats['size'] == 0
        assert stats['inserts'] == 0
        assert stats['updates'] == 0
        assert stats['evictions'] == 0

    def test_stats_after_operations(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1"))
        pool.upsert(make_event("A", "B", "bk1", 2.5, 2.0))
        stats = pool.stats()
        assert stats['inserts'] == 1
        assert stats['updates'] == 1
        assert stats['size'] == 1


class TestEventPoolClear:
    def test_clear(self):
        pool = EventPool()
        pool.upsert(make_event("A", "B", "bk1"))
        pool.clear()
        assert pool.get_count() == 0
