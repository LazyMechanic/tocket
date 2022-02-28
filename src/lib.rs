//! <br>
//!
//! This library provides [`tocket::RateLimiter`][RateLimiter] trait, that provides
//! methods for implementing token bucket rate limiter and
//! [`tocket::InMemoryTokenBucket`][InMemoryTokenBucket] /
//! [`tocket::RedisTokenBucket`][RedisTokenBucket] simple implementations.
//!
//! <br>
//!
//! # Usage example
//!
//! ```no_run
//! use tocket::{RateLimiter, InMemoryTokenBucket};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! # fn main() {
//!     let rl = InMemoryTokenBucket::new(100);
//!     let rl = Arc::new(rl);
//!     
//!     let mut handles = Vec::with_capacity(8);
//!     for _ in 0..8 {
//!         let h = std::thread::spawn({
//!             let rl = Arc::clone(&rl);
//!             move || {
//!                 loop {
//!                     match rl.try_acquire_one() {
//!                         Ok(_) => {
//!                             println!("token acquired, limit not exceeded");
//!                         }
//!                         Err(err) => {
//!                             eprintln!("token acquiring failed: {}", err);
//!                         }
//!                     }
//!                     
//!                     std::thread::sleep(Duration::from_millis(200));
//!                 }
//!             }
//!         });
//!         
//!         handles.push(h);
//!     }
//! # }
//! ```

pub mod error;
pub mod in_memory;
pub mod in_redis;

pub use error::Error;
pub use in_memory::InMemoryTokenBucket;
pub use in_redis::RedisTokenBucket;

/// Trait that provides functionality for acquiring tokens from token bucket.
/// Your type should implement the only `RateLimiter::try_acquire()` function.
pub trait RateLimiter {
    /// Trying acquire tokens from token bucket.
    ///
    /// # Errors
    ///
    /// Will return `Err` if token limit exceeded.
    fn try_acquire(&self, permits: u32) -> Result<(), Error>;

    /// Trying acquire 1 token from token bucket, synonym for `try_acquire(1)`
    ///
    /// # Errors
    ///
    /// Will return `Err` if token limit exceeded.
    fn try_acquire_one(&self) -> Result<(), Error> {
        self.try_acquire(1)
    }
}
