pub mod error;
pub mod in_memory;

pub use error::Error;
pub use in_memory::InMemoryTokenBucket;

pub trait RateLimiter {
    fn try_acquire(&self, permits: u32) -> Result<(), Error>;

    fn try_acquire_one(&self) -> Result<(), Error> {
        self.try_acquire(1)
    }
}
