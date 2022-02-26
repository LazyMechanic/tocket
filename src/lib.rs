pub mod error;
pub mod in_memory;
pub mod in_redis;

pub use error::Error;
pub use in_memory::InMemoryTokenBucket;
pub use in_redis::RedisTokenBucket;

pub trait RateLimiter {
    fn try_acquire(&self, permits: u32) -> Result<(), Error>;

    fn try_acquire_one(&self) -> Result<(), Error> {
        self.try_acquire(1)
    }
}
