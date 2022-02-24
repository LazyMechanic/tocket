use criterion::{black_box, criterion_group, criterion_main, Bencher, Criterion};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Barrier;
use std::time::Instant;
use tocket::{InMemoryTokenBucket, RateLimiter};
use wg::WaitGroup;

fn bench_rate_limiter<T>(b: &mut Bencher, rl: T, target_rps: u32)
where
    T: RateLimiter,
{
    b.iter(|| {
        for _ in 0..target_rps {
            let _ = black_box(rl.try_acquire(1));
        }
    });
}

fn in_memory(c: &mut Criterion) {
    let mut g = c.benchmark_group("InMemoryTokenBucket");

    g.bench_function("in_memory_rps_100_target_100", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(100), 100)
    });

    g.bench_function("in_memory_rps_100_target_200", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(100), 200)
    });

    g.bench_function("in_memory_rps_200_target_200", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(200), 200)
    });

    g.bench_function("in_memory_rps_100_target_300", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(100), 300)
    });

    g.bench_function("in_memory_rps_300_target_300", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(300), 300)
    });

    g.bench_function("in_memory_rps_500_target_1000", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(500), 1000)
    });

    g.bench_function("in_memory_rps_1000_target_1000", |b| {
        bench_rate_limiter(b, InMemoryTokenBucket::new(1000), 1000)
    });

    g.finish();
}

criterion_group! {
    name = bench;
    config = Criterion::default();
    targets = in_memory,
}

criterion_main! {
    bench,
}
