pub mod error;
pub mod in_memory;
pub mod in_redis;

pub use error::Error;
pub use in_memory::InMemoryTokenBucket;
pub use in_redis::RedisTokenBucket;

/// Trait that provides functionality for acquiring tokens from token bucket.
/// Your type should implement the only `RateLimiter::try_acquire()` function.
pub trait RateLimiter {
    /// Trying acquire tokens from token bucket. If limit not exceeded then returns `Ok(())`
    /// otherwise returns `Err(_)`
    fn try_acquire(&self, permits: u32) -> Result<(), Error>;

    /// Trying acquire 1 token from token bucket, synonym for `try_acquire(1)`
    fn try_acquire_one(&self) -> Result<(), Error> {
        self.try_acquire(1)
    }
}
