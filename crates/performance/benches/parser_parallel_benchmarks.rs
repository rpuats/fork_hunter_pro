use criterion::{black_box, criterion_group, criterion_main, Criterion};
use performance::parser_executor::{ParallelParserExecutor, ParserExecutionConfig};

fn parser_single_execution_benchmark(c: &mut Criterion) {
    c.bench_function("parser_single_execution", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("single_parser", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    let result = executor
                        .execute_with_retry("parser1".to_string(), || async {
                            Ok::<_, anyhow::Error>((100, 200))
                        })
                        .await;
                    black_box(result)
                });
            });
    });
}

fn parser_parallel_execution_benchmark(c: &mut Criterion) {
    c.bench_function("parser_parallel_execution_8", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("8_parsers_parallel", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig {
                        max_concurrent_parsers: 8,
                        ..Default::default()
                    });

                    let tasks = (0..8)
                        .map(|i| {
                            (
                                format!("parser_{}", i),
                                || async { Ok::<_, anyhow::Error>((100 + i * 10, 200 + i * 20)) },
                            )
                        })
                        .collect();

                    executor.execute_parallel(black_box(tasks)).await
                });
            });
    });
}

fn parser_parallel_execution_16_benchmark(c: &mut Criterion) {
    c.bench_function("parser_parallel_execution_16", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("16_parsers_parallel", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig {
                        max_concurrent_parsers: 16,
                        ..Default::default()
                    });

                    let tasks = (0..16)
                        .map(|i| {
                            (
                                format!("parser_{}", i),
                                || async { Ok::<_, anyhow::Error>((100 + i * 10, 200 + i * 20)) },
                            )
                        })
                        .collect();

                    executor.execute_parallel(black_box(tasks)).await
                });
            });
    });
}

fn parser_with_retry_success_benchmark(c: &mut Criterion) {
    c.bench_function("parser_retry_success_first_attempt", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("retry_success", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    executor
                        .execute_with_retry("parser".to_string(), || async {
                            Ok::<_, anyhow::Error>((100, 200))
                        })
                        .await
                });
            });
    });
}

fn parser_stats_tracking_benchmark(c: &mut Criterion) {
    c.bench_function("parser_stats_query", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("stats_retrieval", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    
                    for i in 0..10 {
                        executor
                            .execute_with_retry(format!("parser_{}", i), || async {
                                Ok::<_, anyhow::Error>((100, 200))
                            })
                            .await;
                    }
                    
                    black_box(executor.stats())
                });
            });
    });
}

fn parser_history_access_benchmark(c: &mut Criterion) {
    c.bench_function("parser_history_access", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("history_retrieval", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    
                    executor
                        .execute_with_retry("parser1".to_string(), || async {
                            Ok::<_, anyhow::Error>((100, 200))
                        })
                        .await;
                    
                    black_box(executor.history(black_box("parser1")))
                });
            });
    });
}

fn parser_success_rate_calculation_benchmark(c: &mut Criterion) {
    c.bench_function("parser_success_rate", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("success_rate_calc", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    
                    for _ in 0..5 {
                        executor
                            .execute_with_retry("parser1".to_string(), || async {
                                Ok::<_, anyhow::Error>((100, 200))
                            })
                            .await;
                    }
                    
                    black_box(executor.parser_success_rate(black_box("parser1")))
                });
            });
    });
}

fn parser_avg_duration_benchmark(c: &mut Criterion) {
    c.bench_function("parser_avg_duration", |b| {
        b.to_async(tokio::runtime::Runtime::new().unwrap())
            .bench_function("avg_duration_calc", |b| {
                b.iter(|| async {
                    let executor = ParallelParserExecutor::new(ParserExecutionConfig::default());
                    
                    executor
                        .execute_with_retry("parser1".to_string(), || async {
                            Ok::<_, anyhow::Error>((100, 200))
                        })
                        .await;
                    
                    black_box(executor.parser_avg_duration(black_box("parser1")))
                });
            });
    });
}

criterion_group!(
    benches,
    parser_single_execution_benchmark,
    parser_parallel_execution_benchmark,
    parser_parallel_execution_16_benchmark,
    parser_with_retry_success_benchmark,
    parser_stats_tracking_benchmark,
    parser_history_access_benchmark,
    parser_success_rate_calculation_benchmark,
    parser_avg_duration_benchmark,
);

criterion_main!(benches);
