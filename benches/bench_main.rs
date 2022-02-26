use criterion::{black_box, criterion_group, criterion_main, BatchSize, Bencher, Criterion};
use crossbeam::sync::WaitGroup;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use tocket::in_redis::RedisTokenBucket;
use tocket::{InMemoryTokenBucket, RateLimiter};

fn bench_in_memory(b: &mut Bencher, rps: u32, target_rps: u32) {
    b.iter_batched(
        || InMemoryTokenBucket::new(rps),
        |rl| {
            for _ in 0..target_rps {
                let _ = black_box(rl.try_acquire(1));
            }
        },
        BatchSize::SmallInput,
    );
}

fn bench_in_memory_mt(b: &mut Bencher, rps: u32, target_rps: u32, threads_num: u32) {
    b.iter_batched(
        || {
            let rl = InMemoryTokenBucket::new(rps);
            let rl = Arc::new(rl);
            let (starter, waiter) = make_threads(rl, target_rps, threads_num);
            (starter, waiter)
        },
        |(starter, waiter)| {
            starter.start();
            waiter.wait();
        },
        BatchSize::SmallInput,
    );
}

fn bench_redis(b: &mut Bencher, rps: u32, target_rps: u32) {
    b.iter_batched(
        || make_redis_token_bucket(rps),
        |rl| {
            for _ in 0..target_rps {
                let _ = black_box(rl.try_acquire(1));
            }
        },
        BatchSize::SmallInput,
    );
}

fn bench_redis_mt(b: &mut Bencher, rps: u32, target_rps: u32, threads_num: u32) {
    b.iter_batched(
        || {
            let rl = make_redis_token_bucket(rps);
            let rl = Arc::new(rl);
            let (starter, waiter) = make_threads(rl, target_rps, threads_num);
            (starter, waiter)
        },
        |(starter, waiter)| {
            starter.start();
            waiter.wait();
        },
        BatchSize::SmallInput,
    );
}

fn next_bench_redis_namespace() -> String {
    static CNTR: AtomicUsize = AtomicUsize::new(0);
    format!("{}", CNTR.fetch_add(1, Ordering::Relaxed))
}

fn make_redis_token_bucket(rps: u32) -> RedisTokenBucket {
    let namespace = next_bench_redis_namespace();
    let available_tokens_key = format!("{}:available_tokens", namespace);
    let last_refill_key = format!("{}:last_refill", namespace);

    let host = std::env::var("REDIS_HOST").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned());
    let client = redis::Client::open(host).unwrap();

    RedisTokenBucket::new(
        client.get_connection().unwrap(),
        rps,
        available_tokens_key.clone(),
        last_refill_key.clone(),
    )
}

struct Starter {
    flag: Arc<AtomicBool>,
    handlers: Vec<JoinHandle<()>>,
}

impl Starter {
    fn start(self) {
        self.flag.store(true, Ordering::Release);
        for h in self.handlers {
            h.thread().unpark();
        }
    }
}

struct Waiter {
    wg: WaitGroup,
}

impl Waiter {
    fn wait(self) {
        self.wg.wait()
    }
}

fn make_threads<T>(rl: Arc<T>, target_rps: u32, threads_num: u32) -> (Starter, Waiter)
where
    T: RateLimiter + Send + Sync + 'static,
{
    let mut handlers = Vec::with_capacity(threads_num as usize);

    let flag = Arc::new(AtomicBool::new(false));
    let wg = WaitGroup::new();

    for _ in 0..threads_num {
        let rl = Arc::clone(&rl);
        let flag = Arc::clone(&flag);
        let wg = wg.clone();
        let target_rps = target_rps / threads_num;
        let h = std::thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                std::thread::park();
            }

            for _ in 0..target_rps {
                let _ = black_box(rl.try_acquire(1));
            }

            drop(wg);
        });
        handlers.push(h);
    }

    (Starter { flag, handlers }, Waiter { wg })
}

fn bench_acquire_one(c: &mut Criterion) {
    let mut g = c.benchmark_group("acquire_one");

    g.bench_function("in_memory", |b| bench_in_memory(b, 1000, 1));
    g.bench_function("redis", |b| bench_redis(b, 1000, 1));

    g.finish();
}

fn bench_within_limit(c: &mut Criterion) {
    let mut g = c.benchmark_group("within_limit_rps_1000_target_500");

    g.bench_function("in_memory", |b| bench_in_memory(b, 1000, 500));
    g.bench_function("redis", |b| bench_redis(b, 1000, 500));

    g.finish();
}

fn bench_on_limit(c: &mut Criterion) {
    let mut g = c.benchmark_group("on_limit_rps_1000_target_1000");

    g.bench_function("in_memory", |b| bench_in_memory(b, 1000, 1000));
    g.bench_function("redis", |b| bench_redis(b, 1000, 1000));

    g.finish();
}

fn bench_over_limit(c: &mut Criterion) {
    let mut g = c.benchmark_group("over_limit_rps_1000_target_1500");

    g.bench_function("in_memory", |b| bench_in_memory(b, 1000, 1500));
    g.bench_function("redis", |b| bench_redis(b, 1000, 1500));

    g.finish();
}

fn bench_within_limit_mt(c: &mut Criterion) {
    let mut g = c.benchmark_group("within_limit_rps_1000_target_500_multithread");

    // 2 threads
    g.bench_function("in_memory_mt_2", |b| bench_in_memory_mt(b, 1000, 500, 2));
    g.bench_function("redis_mt_2", |b| bench_redis_mt(b, 1000, 500, 2));

    // 4 threads
    g.bench_function("in_memory_mt_4", |b| bench_in_memory_mt(b, 1000, 500, 4));
    g.bench_function("redis_mt_4", |b| bench_redis_mt(b, 1000, 500, 4));

    // 8 threads
    g.bench_function("in_memory_mt_8", |b| bench_in_memory_mt(b, 1000, 500, 8));
    g.bench_function("redis_mt_8", |b| bench_redis_mt(b, 1000, 500, 8));

    // 16 threads
    g.bench_function("in_memory_mt_16", |b| bench_in_memory_mt(b, 1000, 500, 16));
    g.bench_function("redis_mt_16", |b| bench_redis_mt(b, 1000, 500, 16));

    // 32 threads
    g.bench_function("in_memory_mt_32", |b| bench_in_memory_mt(b, 1000, 500, 32));
    g.bench_function("redis_mt_32", |b| bench_redis_mt(b, 1000, 500, 32));

    g.finish();
}

fn bench_on_limit_mt(c: &mut Criterion) {
    let mut g = c.benchmark_group("on_limit_rps_1000_target_1000_multithread");

    // 2 threads
    g.bench_function("in_memory_mt_2", |b| bench_in_memory_mt(b, 1000, 1000, 2));
    g.bench_function("redis_mt_2", |b| bench_redis_mt(b, 1000, 1000, 2));

    // 4 threads
    g.bench_function("in_memory_mt_4", |b| bench_in_memory_mt(b, 1000, 1000, 4));
    g.bench_function("redis_onl_mt_4", |b| bench_redis_mt(b, 1000, 1000, 4));

    // 8 threads
    g.bench_function("in_memory_mt_8", |b| bench_in_memory_mt(b, 1000, 1000, 8));
    g.bench_function("redis_mt_8", |b| bench_redis_mt(b, 1000, 1000, 8));

    // 16 threads
    g.bench_function("in_memory_mt_16", |b| bench_in_memory_mt(b, 1000, 1000, 16));
    g.bench_function("redis_mt_16", |b| bench_redis_mt(b, 1000, 1000, 16));

    // 32 threads
    g.bench_function("in_memory_mt_32", |b| bench_in_memory_mt(b, 1000, 1000, 32));
    g.bench_function("redis_mt_32", |b| bench_redis_mt(b, 1000, 1000, 32));

    g.finish();
}

fn bench_over_limit_mt(c: &mut Criterion) {
    let mut g = c.benchmark_group("over_limit_rps_1000_target_1500_multithread");

    // 2 threads
    g.bench_function("in_memory_mt_2", |b| bench_in_memory_mt(b, 1000, 1500, 2));
    g.bench_function("redis_mt_2", |b| bench_redis_mt(b, 1000, 1500, 2));

    // 4 threads
    g.bench_function("in_memory_mt_4", |b| bench_in_memory_mt(b, 1000, 1500, 4));
    g.bench_function("redis_mt_4", |b| bench_redis_mt(b, 1000, 1500, 4));

    // 8 threads
    g.bench_function("in_memory_mt_8", |b| bench_in_memory_mt(b, 1000, 1500, 8));
    g.bench_function("redis_mt_8", |b| bench_redis_mt(b, 1000, 1500, 8));

    // 16 threads
    g.bench_function("in_memory_mt_16", |b| bench_in_memory_mt(b, 1000, 1500, 16));
    g.bench_function("redis_mt_16", |b| bench_redis_mt(b, 1000, 1500, 16));

    // 32 threads
    g.bench_function("in_memory_mt_32", |b| bench_in_memory_mt(b, 1000, 1500, 32));
    g.bench_function("redis_mt_32", |b| bench_redis_mt(b, 1000, 1500, 32));

    g.finish();
}

criterion_group! {
    name = bench;
    config = Criterion::default();
    targets = bench_acquire_one,
              bench_within_limit,
              bench_on_limit,
              bench_over_limit,
              bench_within_limit_mt,
              bench_on_limit_mt,
              bench_over_limit_mt,
}

criterion_main! {
    bench,
}
