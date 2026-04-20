use criterion::{black_box, criterion_group, criterion_main, Criterion};
use performance::batch_calculator::{BatchCalculator, BatchConfig, BatchRequest};

fn batch_add_single_request_benchmark(c: &mut Criterion) {
    c.bench_function("batch_add_single_request", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());
        let request = BatchRequest {
            id: "1".to_string(),
            event_id: "evt1".to_string(),
            odds: vec![2.0, 2.0],
            bookmaker: "bk1".to_string(),
            market: "1x2".to_string(),
        };

        b.iter(|| {
            calc.add_request(black_box(request.clone()))
        });
    });
}

fn batch_add_multiple_requests_benchmark(c: &mut Criterion) {
    c.bench_function("batch_add_100_requests", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());
        let requests = (0..100)
            .map(|i| BatchRequest {
                id: format!("{}", i),
                event_id: format!("evt_{}", i),
                odds: vec![2.0, 2.0],
                bookmaker: "bk1".to_string(),
                market: "1x2".to_string(),
            })
            .collect::<Vec<_>>();

        b.iter(|| {
            calc.add_requests(black_box(requests.clone()))
        });
    });
}

fn batch_calculate_odds_benchmark(c: &mut Criterion) {
    c.bench_function("batch_calculate_odds", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        b.iter(|| {
            calc.calculate_odds(black_box(&[2.0, 2.0]))
        });
    });
}

fn batch_calculate_odds_3way_benchmark(c: &mut Criterion) {
    c.bench_function("batch_calculate_odds_3way", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        b.iter(|| {
            calc.calculate_odds(black_box(&[2.0, 3.0, 2.5]))
        });
    });
}

fn batch_process_single_batch_benchmark(c: &mut Criterion) {
    c.bench_function("batch_process_single_batch", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("process_batch", |b| {
                b.iter(|| async {
                    let calc = BatchCalculator::new(BatchConfig::default());

                    for i in 0..10 {
                        calc.add_request(BatchRequest {
                            id: format!("{}", i),
                            event_id: format!("evt_{}", i),
                            odds: vec![2.0, 2.0],
                            bookmaker: "bk1".to_string(),
                            market: "1x2".to_string(),
                        });
                    }

                    black_box(calc.process_batch().await)
                });
            });
    });
}

fn batch_cache_hit_benchmark(c: &mut Criterion) {
    c.bench_function("batch_cache_hit", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());
        let cached = calc.get_cached("evt1", &[2.0, 2.0]);
        
        b.iter(|| {
            black_box(calc.get_cached(black_box("evt1"), black_box(&[2.0, 2.0])))
        });
    });
}

fn batch_cache_clear_benchmark(c: &mut Criterion) {
    c.bench_function("batch_cache_clear", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        // Populate cache
        for i in 0..100 {
            calc.add_request(BatchRequest {
                id: format!("{}", i),
                event_id: format!("evt_{}", i),
                odds: vec![2.0, 2.0],
                bookmaker: "bk1".to_string(),
                market: "1x2".to_string(),
            });
        }

        b.iter(|| {
            calc.clear_cache()
        });
    });
}

fn batch_stats_retrieval_benchmark(c: &mut Criterion) {
    c.bench_function("batch_stats_retrieval", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        b.iter(|| {
            black_box(calc.stats())
        });
    });
}

fn batch_pending_size_benchmark(c: &mut Criterion) {
    c.bench_function("batch_pending_size", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        for i in 0..50 {
            calc.add_request(BatchRequest {
                id: format!("{}", i),
                event_id: format!("evt_{}", i),
                odds: vec![2.0, 2.0],
                bookmaker: "bk1".to_string(),
                market: "1x2".to_string(),
            });
        }

        b.iter(|| {
            black_box(calc.pending_size())
        });
    });
}

fn batch_cache_size_benchmark(c: &mut Criterion) {
    c.bench_function("batch_cache_size", |b| {
        let calc = BatchCalculator::new(BatchConfig::default());

        b.iter(|| {
            black_box(calc.cache_size())
        });
    });
}

criterion_group!(
    benches,
    batch_add_single_request_benchmark,
    batch_add_multiple_requests_benchmark,
    batch_calculate_odds_benchmark,
    batch_calculate_odds_3way_benchmark,
    batch_process_single_batch_benchmark,
    batch_cache_hit_benchmark,
    batch_cache_clear_benchmark,
    batch_stats_retrieval_benchmark,
    batch_pending_size_benchmark,
    batch_cache_size_benchmark,
);

criterion_main!(benches);
