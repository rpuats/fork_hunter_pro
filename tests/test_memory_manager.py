# tests/test_memory_manager.py
import pytest
import sys
import os
import gc
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.memory_manager import MemoryManager, ObjectPool, MemorySnapshot


class TestObjectPool:
    def test_create_pool(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        assert pool._max_size == 10
        assert pool._name == "test"

    def test_acquire_creates_new(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        obj = pool.acquire()
        assert isinstance(obj, dict)
        assert pool._created == 1
        assert pool._reused == 0

    def test_release_and_reuse(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        obj1 = pool.acquire()
        pool.release(obj1)
        obj2 = pool.acquire()
        assert pool._reused == 1
        assert pool._created == 1

    def test_max_size_respected(self):
        pool = ObjectPool(factory=dict, max_size=2, name="test")
        obj1 = pool.acquire()
        obj2 = pool.acquire()
        obj3 = pool.acquire()
        pool.release(obj1)
        pool.release(obj2)
        pool.release(obj3)
        assert len(pool._pool) == 2

    def test_stats(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        pool.acquire()
        pool.acquire()
        stats = pool.stats()
        assert stats['name'] == "test"
        assert stats['created'] == 2
        assert stats['pool_size'] == 0
        assert stats['max_size'] == 10

    def test_stats_reuse_rate(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        obj = pool.acquire()
        pool.release(obj)
        pool.acquire()
        stats = pool.stats()
        assert stats['reused'] == 1
        assert stats['reuse_rate'] == 50.0

    def test_clear(self):
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        obj = pool.acquire()
        pool.release(obj)
        assert len(pool._pool) == 1
        pool.clear()
        assert len(pool._pool) == 0

    def test_custom_factory(self):
        pool = ObjectPool(factory=lambda: {"custom": True}, max_size=5, name="custom")
        obj = pool.acquire()
        assert obj["custom"] is True


class TestMemoryManagerInit:
    def test_default_values(self):
        mm = MemoryManager()
        assert mm.memory_limit_mb == 512.0
        assert mm.cleanup_threshold_mb == 400.0
        assert mm.snapshot_interval == 100

    def test_custom_values(self):
        mm = MemoryManager(
            memory_limit_mb=256.0,
            cleanup_threshold_mb=200.0,
            snapshot_interval=50,
        )
        assert mm.memory_limit_mb == 256.0
        assert mm.cleanup_threshold_mb == 200.0
        assert mm.snapshot_interval == 50


class TestMemoryManagerLifecycle:
    def test_start(self):
        mm = MemoryManager()
        mm.start()
        assert mm._started is True
        mm.stop()

    def test_stop(self):
        mm = MemoryManager()
        mm.start()
        mm.stop()
        assert mm._started is False

    def test_start_idempotent(self):
        mm = MemoryManager()
        mm.start()
        mm.start()
        assert mm._started is True
        mm.stop()

    def test_stop_idempotent(self):
        mm = MemoryManager()
        mm.start()
        mm.stop()
        mm.stop()
        assert mm._started is False


class TestMemoryManagerOnCycle:
    def test_cycle_count_increments(self):
        mm = MemoryManager()
        mm.start()
        mm.on_cycle()
        assert mm._cycle_count == 1
        mm.stop()

    def test_snapshot_taken_at_interval(self):
        mm = MemoryManager(snapshot_interval=1)
        mm.start()
        mm.on_cycle()
        assert len(mm._snapshots) == 1
        mm.stop()

    def test_no_snapshot_before_interval(self):
        mm = MemoryManager(snapshot_interval=10)
        mm.start()
        for _ in range(5):
            mm.on_cycle()
        assert len(mm._snapshots) == 0
        mm.stop()

    def test_no_snapshot_if_not_started(self):
        mm = MemoryManager(snapshot_interval=1)
        mm.on_cycle()
        assert len(mm._snapshots) == 0


class TestMemoryManagerMemoryUsage:
    def test_get_memory_usage_started(self):
        mm = MemoryManager()
        mm.start()
        usage = mm.get_memory_usage()
        assert 'current_mb' in usage
        assert 'peak_mb' in usage
        assert 'limit_mb' in usage
        assert 'utilization_percent' in usage
        assert 'gc_objects' in usage
        assert 'gc_generations' in usage
        mm.stop()

    def test_get_memory_usage_not_started(self):
        mm = MemoryManager()
        usage = mm.get_memory_usage()
        assert usage == {'current_mb': 0, 'peak_mb': 0}

    def test_get_top_allocations_started(self):
        mm = MemoryManager()
        mm.start()
        allocs = mm.get_top_allocations(count=5)
        assert isinstance(allocs, list)
        assert len(allocs) <= 5
        mm.stop()

    def test_get_top_allocations_not_started(self):
        mm = MemoryManager()
        allocs = mm.get_top_allocations()
        assert allocs == []


class TestMemoryManagerPools:
    def test_register_pool(self):
        mm = MemoryManager()
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        mm.register_pool("test", pool)
        assert "test" in mm._object_pools

    def test_get_pool_stats(self):
        mm = MemoryManager()
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        pool.acquire()
        mm.register_pool("test", pool)
        stats = mm.get_pool_stats()
        assert "test" in stats
        assert stats["test"]["created"] == 1


class TestMemoryManagerCallbacks:
    def test_register_callback(self):
        mm = MemoryManager()
        called = []
        mm.register_callback(lambda event, mb: called.append((event, mb)))
        assert len(mm._callbacks) == 1

    def test_callback_on_memory_limit_exceeded(self):
        mm = MemoryManager(memory_limit_mb=0.001)
        mm.start()
        called = []
        mm.register_callback(lambda event, mb: called.append((event, mb)))
        mm._force_cleanup()
        current, _ = __import__('tracemalloc').get_traced_memory()
        current_mb = current / 1024 / 1024
        if current_mb > mm.memory_limit_mb:
            mm._check_memory_limits()
            assert len(called) >= 1
            assert called[0][0] == 'memory_limit_exceeded'
        mm.stop()


class TestMemoryManagerReport:
    def test_get_report(self):
        mm = MemoryManager()
        mm.start()
        mm.on_cycle()
        report = mm.get_report()
        assert 'memory' in report
        assert 'pools' in report
        assert 'cleanups' in report
        assert 'cycles' in report
        assert 'snapshots' in report
        mm.stop()


class TestMemoryManagerCleanup:
    def test_force_cleanup(self):
        mm = MemoryManager()
        mm.start()
        pool = ObjectPool(factory=dict, max_size=10, name="test")
        pool.acquire()
        mm.register_pool("test", pool)
        initial_count = mm._cleanup_count
        mm._force_cleanup()
        assert mm._cleanup_count == initial_count + 1
        assert len(pool._pool) == 0
        mm.stop()

    def test_cleanup_threshold_triggers_cleanup(self):
        mm = MemoryManager(cleanup_threshold_mb=0.001, memory_limit_mb=1000.0)
        mm.start()
        initial_count = mm._cleanup_count
        current, _ = __import__('tracemalloc').get_traced_memory()
        current_mb = current / 1024 / 1024
        if current_mb > mm.cleanup_threshold_mb:
            mm._check_memory_limits()
            assert mm._cleanup_count > initial_count
        else:
            mm._force_cleanup()
            assert mm._cleanup_count > initial_count
        mm.stop()


class TestMemorySnapshot:
    def test_create_snapshot(self):
        snap = MemorySnapshot(
            timestamp=time.time(),
            current_mb=10.0,
            peak_mb=20.0,
            gc_objects=1000,
        )
        assert snap.current_mb == 10.0
        assert snap.peak_mb == 20.0
        assert snap.gc_objects == 1000
        assert snap.top_allocations == []
