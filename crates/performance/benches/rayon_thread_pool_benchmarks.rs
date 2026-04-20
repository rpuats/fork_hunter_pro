use criterion::{black_box, criterion_group, criterion_main, Criterion};
use performance::thread_pool::{ThreadPoolExecutor, ThreadPoolConfig};

fn thread_pool_map_small_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_map_100", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..100).collect::<Vec<_>>();

        b.iter(|| {
            pool.map_parallel(black_box(items.clone()), |x| x * 2)
        });
    });
}

fn thread_pool_map_medium_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_map_1000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..1000).collect::<Vec<_>>();

        b.iter(|| {
            pool.map_parallel(black_box(items.clone()), |x| x * 2)
        });
    });
}

fn thread_pool_map_large_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_map_10000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..10000).collect::<Vec<_>>();

        b.iter(|| {
            pool.map_parallel(black_box(items.clone()), |x| x * 2)
        });
    });
}

fn thread_pool_filter_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_filter_1000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..1000).collect::<Vec<_>>();

        b.iter(|| {
            pool.filter_parallel(black_box(items.clone()), |x| x % 2 == 0)
        });
    });
}

fn thread_pool_sum_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_sum_10000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0i64..10000).collect::<Vec<_>>();

        b.iter(|| {
            pool.sum_parallel(black_box(items.clone()))
        });
    });
}

fn thread_pool_average_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_average_5000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..5000).map(|i| i as f64).collect::<Vec<_>>();

        b.iter(|| {
            pool.average_parallel(black_box(items.clone()))
        });
    });
}

fn thread_pool_sort_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_sort_1000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();

        b.iter(|| {
            let items = (0..1000).rev().collect::<Vec<_>>();
            pool.sort_parallel(black_box(items))
        });
    });
}

fn thread_pool_sort_large_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_sort_10000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();

        b.iter(|| {
            let items = (0..10000).rev().collect::<Vec<_>>();
            pool.sort_parallel(black_box(items))
        });
    });
}

fn thread_pool_for_each_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_for_each_1000", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..1000).collect::<Vec<_>>();

        b.iter(|| {
            pool.for_each_parallel(black_box(items.clone()), |x| {
                let _ = black_box(x * 2);
            });
        });
    });
}

fn thread_pool_stats_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_stats_retrieval", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();

        b.iter(|| {
            black_box(pool.stats())
        });
    });
}

fn thread_pool_group_by_benchmark(c: &mut Criterion) {
    c.bench_function("thread_pool_group_by_100", |b| {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = (0..100).collect::<Vec<_>>();

        b.iter(|| {
            pool.group_by_parallel(black_box(items.clone()), |x| x / 10)
        });
    });
}

criterion_group!(
    benches,
    thread_pool_map_small_benchmark,
    thread_pool_map_medium_benchmark,
    thread_pool_map_large_benchmark,
    thread_pool_filter_benchmark,
    thread_pool_sum_benchmark,
    thread_pool_average_benchmark,
    thread_pool_sort_benchmark,
    thread_pool_sort_large_benchmark,
    thread_pool_for_each_benchmark,
    thread_pool_stats_benchmark,
    thread_pool_group_by_benchmark,
);

criterion_main!(benches);
