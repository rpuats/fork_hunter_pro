use criterion::{black_box, criterion_group, criterion_main, Criterion};
use performance::cache::{SmartCache, CacheConfig};

fn cache_put_benchmark(c: &mut Criterion) {
    c.bench_function("cache_put_single", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        b.iter(|| {
            cache.put(
                black_box("key".to_string()),
                black_box(vec![1, 2, 3, 4, 5]),
                None,
            )
        });
    });
}

fn cache_get_benchmark(c: &mut Criterion) {
    c.bench_function("cache_get_hit", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key".to_string(), vec![1, 2, 3, 4, 5], None);
        
        b.iter(|| {
            cache.get(black_box("key"))
        });
    });
}

fn cache_get_miss_benchmark(c: &mut Criterion) {
    c.bench_function("cache_get_miss", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        
        b.iter(|| {
            cache.get(black_box("nonexistent"))
        });
    });
}

fn cache_batch_put_benchmark(c: &mut Criterion) {
    c.bench_function("cache_batch_put_100", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        let entries = (0..100)
            .map(|i| (format!("key_{}", i), vec![i as u8; 100]))
            .collect::<Vec<_>>();
        
        b.iter(|| {
            cache.batch_put(black_box(entries.clone()), None);
        });
    });
}

fn cache_batch_get_benchmark(c: &mut Criterion) {
    c.bench_function("cache_batch_get_50_hits", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        for i in 0..50 {
            cache.put(format!("key_{}", i), vec![i as u8; 100], None);
        }
        
        let keys = (0..50).map(|i| format!("key_{}", i)).collect::<Vec<_>>();
        
        b.iter(|| {
            cache.batch_get(black_box(&keys))
        });
    });
}

fn cache_json_operations_benchmark(c: &mut Criterion) {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestData {
        value: i32,
    }

    c.bench_function("cache_json_put_get", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        let data = TestData { value: 42 };
        
        b.iter(|| {
            cache.put_json("key".to_string(), &black_box(&data), None);
            cache.get_json::<TestData>(black_box("key"))
        });
    });
}

fn cache_lru_eviction_benchmark(c: &mut Criterion) {
    c.bench_function("cache_lru_eviction", |b| {
        let config = CacheConfig {
            max_size: 100,
            ttl_seconds: 300,
            update_interval_ms: 100,
        };
        let cache = SmartCache::new(config);
        
        b.iter(|| {
            for i in 0..150 {
                cache.put(
                    format!("key_{}", i),
                    vec![i as u8; 50],
                    None,
                );
            }
        });
    });
}

fn cache_cleanup_expired_benchmark(c: &mut Criterion) {
    c.bench_function("cache_cleanup_expired", |b| {
        let config = CacheConfig {
            max_size: 10000,
            ttl_seconds: 1,
            update_interval_ms: 100,
        };
        let cache = SmartCache::new(config);
        
        for i in 0..1000 {
            cache.put(format!("key_{}", i), vec![i as u8; 100], Some(1));
        }
        
        b.iter(|| {
            cache.cleanup_expired()
        });
    });
}

fn cache_memory_usage_benchmark(c: &mut Criterion) {
    c.bench_function("cache_memory_usage", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        
        for i in 0..1000 {
            cache.put(format!("key_{}", i), vec![i as u8; 100], None);
        }
        
        b.iter(|| {
            cache.memory_usage()
        });
    });
}

fn cache_ttl_update_benchmark(c: &mut Criterion) {
    c.bench_function("cache_update_ttl", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key".to_string(), vec![1, 2, 3], None);
        
        b.iter(|| {
            cache.update_ttl(black_box("key"), black_box(600))
        });
    });
}

fn cache_remaining_ttl_benchmark(c: &mut Criterion) {
    c.bench_function("cache_remaining_ttl", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key".to_string(), vec![1, 2, 3], Some(300));
        
        b.iter(|| {
            cache.remaining_ttl(black_box("key"))
        });
    });
}

fn cache_pattern_matching_benchmark(c: &mut Criterion) {
    c.bench_function("cache_keys_matching", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        
        for i in 0..100 {
            cache.put(format!("event_{}_{}", i / 10, i), vec![1, 2, 3], None);
        }
        
        b.iter(|| {
            cache.keys_matching(black_box("event_5"))
        });
    });
}

fn cache_entry_touch_benchmark(c: &mut Criterion) {
    c.bench_function("cache_entry_touch", |b| {
        let cache = SmartCache::new(CacheConfig::default());
        cache.put("key".to_string(), vec![1, 2, 3], None);
        
        b.iter(|| {
            let _ = cache.get(black_box("key"));
        });
    });
}

criterion_group!(
    benches,
    cache_put_benchmark,
    cache_get_benchmark,
    cache_get_miss_benchmark,
    cache_batch_put_benchmark,
    cache_batch_get_benchmark,
    cache_json_operations_benchmark,
    cache_lru_eviction_benchmark,
    cache_cleanup_expired_benchmark,
    cache_memory_usage_benchmark,
    cache_ttl_update_benchmark,
    cache_remaining_ttl_benchmark,
    cache_pattern_matching_benchmark,
    cache_entry_touch_benchmark,
);

criterion_main!(benches);
