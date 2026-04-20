//! Thread pool executor for CPU-bound operations
//! Uses rayon for data parallelism and efficient CPU utilization

use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info};

/// Thread pool configuration
#[derive(Debug, Clone)]
pub struct ThreadPoolConfig {
    pub num_threads: usize,
    pub queue_size: usize,
    pub thread_stack_size: Option<usize>,
}

impl Default for ThreadPoolConfig {
    fn default() -> Self {
        Self {
            num_threads: num_cpus::get(),
            queue_size: 10000,
            thread_stack_size: None,
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub total_threads: usize,
    pub active_threads: usize,
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
}

impl PoolStats {
    pub fn utilization_percent(&self) -> f64 {
        if self.total_threads == 0 {
            0.0
        } else {
            (self.active_threads as f64 / self.total_threads as f64) * 100.0
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.completed_tasks + self.failed_tasks;
        if total == 0 {
            0.0
        } else {
            self.completed_tasks as f64 / total as f64
        }
    }
}

/// Thread pool executor using rayon
pub struct ThreadPoolExecutor {
    config: RwLock<ThreadPoolConfig>,
    stats: RwLock<PoolStats>,
    thread_pool: rayon::ThreadPool,
}

impl ThreadPoolExecutor {
    /// Create a new thread pool executor
    pub fn new(config: ThreadPoolConfig) -> anyhow::Result<Self> {
        info!(
            "Creating ThreadPoolExecutor with {} threads",
            config.num_threads
        );

        let mut builder = rayon::ThreadPoolBuilder::new()
            .num_threads(config.num_threads)
            .stack_size(config.thread_stack_size.unwrap_or(1024 * 1024));

        let thread_pool = builder.build()?;

        Ok(Self {
            config: RwLock::new(config),
            stats: RwLock::new(PoolStats {
                total_threads: config.num_threads,
                active_threads: 0,
                total_tasks: 0,
                completed_tasks: 0,
                failed_tasks: 0,
            }),
            thread_pool,
        })
    }

    /// Execute a task in the thread pool
    pub fn execute<F, R>(&self, task: F) -> R
    where
        F: FnOnce() -> anyhow::Result<R> + Send,
        R: Send,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        stats.active_threads = std::cmp::min(stats.total_threads, stats.active_threads + 1);
        drop(stats);

        self.thread_pool.install(|| {
            match task() {
                Ok(result) => {
                    let mut stats = self.stats.write();
                    stats.completed_tasks += 1;
                    stats.active_threads = stats.active_threads.saturating_sub(1);
                    result
                }
                Err(e) => {
                    let mut stats = self.stats.write();
                    stats.failed_tasks += 1;
                    stats.active_threads = stats.active_threads.saturating_sub(1);
                    panic!("Task failed: {}", e);
                }
            }
        })
    }

    /// Execute parallel map operation
    pub fn map_parallel<I, O, F>(&self, items: Vec<I>, f: F) -> Vec<O>
    where
        I: Send,
        O: Send,
        F: Fn(I) -> O + Send + Sync,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        drop(stats);

        self.thread_pool.install(|| {
            items
                .into_par_iter()
                .map(f)
                .collect()
        })
    }

    /// Execute parallel filter operation
    pub fn filter_parallel<I, F>(&self, items: Vec<I>, predicate: F) -> Vec<I>
    where
        I: Send,
        F: Fn(&I) -> bool + Send + Sync,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        drop(stats);

        self.thread_pool.install(|| {
            items
                .into_par_iter()
                .filter(predicate)
                .collect()
        })
    }

    /// Execute parallel fold/reduce operation
    pub fn reduce_parallel<I, F, R>(&self, items: Vec<I>, identity: R, op: F) -> R
    where
        I: Send,
        R: Send + Clone,
        F: Fn(R, I) -> R + Send + Sync,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        drop(stats);

        self.thread_pool.install(|| {
            items
                .into_par_iter()
                .fold(|| identity.clone(), op)
                .reduce(|| identity.clone(), |a, b| {
                    // Simple reduction - in real use, would need proper associative operation
                    a
                })
        })
    }

    /// Execute parallel for_each
    pub fn for_each_parallel<I, F>(&self, items: Vec<I>, f: F)
    where
        I: Send,
        F: Fn(I) + Send + Sync,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        drop(stats);

        self.thread_pool.install(|| {
            items.into_par_iter().for_each(f);
        });

        let mut stats = self.stats.write();
        stats.completed_tasks += 1;
    }

    /// Process batch of CPU-intensive tasks
    pub fn process_batch<T, F>(&self, items: Vec<T>, processor: F) -> Vec<anyhow::Result<T>>
    where
        T: Send,
        F: Fn(T) -> anyhow::Result<T> + Send + Sync,
    {
        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        drop(stats);

        self.thread_pool.install(|| {
            items
                .into_par_iter()
                .map(|item| processor(item))
                .collect()
        })
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        *self.stats.read()
    }

    /// Update active thread count
    pub fn set_active_threads(&self, count: usize) {
        let mut stats = self.stats.write();
        stats.active_threads = count;
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = PoolStats {
            total_threads: stats.total_threads,
            active_threads: 0,
            total_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
        };
    }

    /// Get thread count
    pub fn thread_count(&self) -> usize {
        self.config.read().num_threads
    }

    /// Parallel sum operation
    pub fn sum_parallel(&self, items: Vec<i64>) -> i64 {
        self.thread_pool.install(|| {
            items.into_par_iter().sum()
        })
    }

    /// Parallel average calculation
    pub fn average_parallel(&self, items: Vec<f64>) -> f64 {
        if items.is_empty() {
            return 0.0;
        }

        self.thread_pool.install(|| {
            let sum: f64 = items.par_iter().sum();
            sum / items.len() as f64
        })
    }

    /// Parallel sorting
    pub fn sort_parallel<T: Ord + Send>(&self, mut items: Vec<T>) -> Vec<T> {
        self.thread_pool.install(|| {
            items.par_sort();
            items
        })
    }

    /// Parallel group by
    pub fn group_by_parallel<T, K, F>(&self, items: Vec<T>, key_fn: F) 
        -> std::collections::HashMap<K, Vec<T>>
    where
        T: Send,
        K: Eq + std::hash::Hash + Send + Clone,
        F: Fn(&T) -> K + Send + Sync,
    {
        self.thread_pool.install(|| {
            items
                .into_par_iter()
                .fold(
                    || std::collections::HashMap::new(),
                    |mut map, item| {
                        let key = key_fn(&item);
                        map.entry(key).or_insert_with(Vec::new).push(item);
                        map
                    },
                )
                .reduce(
                    || std::collections::HashMap::new(),
                    |mut map1, map2| {
                        for (k, v) in map2 {
                            map1.entry(k).or_insert_with(Vec::new).extend(v);
                        }
                        map1
                    },
                )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_pool_creation() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default());
        assert!(pool.is_ok());
    }

    #[test]
    fn test_parallel_map() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = vec![1, 2, 3, 4, 5];
        let results = pool.map_parallel(items, |x| x * 2);
        
        assert_eq!(results, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_parallel_filter() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = vec![1, 2, 3, 4, 5];
        let mut results = pool.filter_parallel(items, |x| x % 2 == 0);
        results.sort();
        
        assert_eq!(results, vec![2, 4]);
    }

    #[test]
    fn test_parallel_sum() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = vec![1, 2, 3, 4, 5];
        let sum = pool.sum_parallel(items);
        
        assert_eq!(sum, 15);
    }

    #[test]
    fn test_parallel_average() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let items = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let avg = pool.average_parallel(items);
        
        assert_eq!(avg, 3.0);
    }

    #[test]
    fn test_thread_pool_stats() {
        let pool = ThreadPoolExecutor::new(ThreadPoolConfig::default()).unwrap();
        let stats = pool.stats();
        
        assert_eq!(stats.total_threads, num_cpus::get());
        assert_eq!(stats.total_tasks, 0);
    }
}
