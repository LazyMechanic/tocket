//! # Tocket
//!
//! This library provides implementation of token bucket algorithm.
//!
//! ## Usage example
//!
//! ```no_run
//! use tocket::{TokenBucket, InMemoryStorage};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! fn main() {
//!     let tb = TokenBucket::new(InMemoryStorage::new(100));
//!     let tb = Arc::new(tb);
//!     
//!     for _ in 0..8 {
//!         std::thread::spawn({
//!             let tb = Arc::clone(&tb);
//!             move || {
//!                 loop {
//!                     match tb.try_acquire_one() {
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
//!     }
//! }
//! ```

pub mod distributed;
pub mod in_memory;
#[cfg(feature = "redis-impl")]
pub mod in_redis;

pub use distributed::*;
pub use in_memory::*;
#[cfg(feature = "redis-impl")]
pub use in_redis::*;

pub trait Storage {
    type Error: From<RateLimitExceededError>;

    fn try_acquire(&self, alg: TokenBucketAlgorithm, permits: u32) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct State {
    pub cap: u32,
    pub available_tokens: u32,
    pub last_refill: time::OffsetDateTime,
    pub refill_tick: time::Duration,
}

/// Rate limiter that implements token bucket algorithm.
pub struct TokenBucket<S> {
    storage: S,
}

impl<S> TokenBucket<S>
where
    S: Storage,
{
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub fn try_acquire(&self, permits: u32) -> Result<(), S::Error> {
        self.storage
            .try_acquire(TokenBucketAlgorithm { mode: Mode::N }, permits)
    }

    pub fn try_acquire_one(&self) -> Result<(), S::Error> {
        self.try_acquire(1)
    }

    pub fn try_acquire_n_or_all(&self, permits: u32) -> Result<(), S::Error> {
        self.storage
            .try_acquire(TokenBucketAlgorithm { mode: Mode::All }, permits)
    }
}

#[derive(Debug)]
pub struct TokenBucketAlgorithm {
    mode: Mode,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Mode {
    N,
    All,
}

impl TokenBucketAlgorithm {
    pub fn try_acquire(
        &self,
        state: &mut State,
        permits: u32,
    ) -> Result<(), RateLimitExceededError> {
        self.refill_state(state);

        match self.mode {
            Mode::N => {
                if state.available_tokens >= permits {
                    state.available_tokens -= permits;
                    Ok(())
                } else {
                    Err(RateLimitExceededError(()))
                }
            }
            Mode::All => {
                state.available_tokens -= u32::min(permits, state.available_tokens);
                Ok(())
            }
        }
    }

    fn refill_state(&self, state: &mut State) {
        let now = time::OffsetDateTime::now_utc();
        let since_last_refill = now - state.last_refill;

        if since_last_refill <= state.refill_tick {
            return;
        }

        let tokens_since_last_refill = {
            let mut tokens_count = 0u32;
            let mut k = since_last_refill;
            loop {
                k -= state.refill_tick;
                if k <= time::Duration::ZERO {
                    break;
                }
                tokens_count += 1;
            }
            tokens_count
        };

        state.available_tokens =
            u32::min(state.available_tokens + tokens_since_last_refill, state.cap);
        state.last_refill += state.refill_tick * tokens_since_last_refill;
    }
}

#[derive(Debug, thiserror::Error)]
#[error("rate limit exceeded")]
pub struct RateLimitExceededError(());
