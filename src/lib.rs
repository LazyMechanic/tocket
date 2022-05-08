#![cfg_attr(docsrs, feature(doc_cfg))]

//! # Tocket
//!
//! This library provides implementation of token bucket algorithm and some storage implementations.
//!
//! Available storages:
//! - [`InMemoryStorage`]
//! - [`RedisStorage`]
//! - [`DistributedStorage`]
//!
//! You can implement your own [storage] (e.g. Postgres).
//!
//! [`InMemoryStorage`]: crate::in_memory::InMemoryStorage
//! [`RedisStorage`]: crate::in_redis::RedisStorage
//! [`DistributedStorage`]: crate::distributed::DistributedStorage
//! [storage]: crate::Storage

pub mod in_memory;

#[cfg(feature = "distributed-impl")]
#[cfg_attr(docsrs, doc(cfg(feature = "distributed-impl")))]
pub mod distributed;

#[cfg(feature = "redis-impl")]
#[cfg_attr(docsrs, doc(cfg(feature = "redis-impl")))]
pub mod in_redis;

pub use in_memory::*;

#[cfg(feature = "distributed-impl")]
#[cfg_attr(docsrs, doc(cfg(feature = "distributed-impl")))]
pub use distributed::*;

#[cfg(feature = "redis-impl")]
#[cfg_attr(docsrs, doc(cfg(feature = "redis-impl")))]
pub use in_redis::*;

/// Trait that provides function for tokens acquiring.
///
/// Object that implements this trait should load state, execute provided algorithm
/// and save updated state.
pub trait Storage {
    type Error: From<RateLimitExceededError>;

    fn try_acquire(&self, alg: TokenBucketAlgorithm, permits: u32) -> Result<(), Self::Error>;
}

/// State of token bucket.
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
    /// Creates new token bucket rate limiter with provided storage.
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    /// Tries to acquire N tokens.
    ///
    /// # Errors
    ///
    /// Will return `Err` if there are not enough tokens or if the storage could not save/load state.
    pub fn try_acquire(&self, permits: u32) -> Result<(), S::Error> {
        self.storage
            .try_acquire(TokenBucketAlgorithm { mode: Mode::N }, permits)
    }

    /// Tries to acquire 1 token.
    ///
    /// # Errors
    ///
    /// Will return `Err` if there are not enough tokens or if the storage could not save/load state.
    pub fn try_acquire_one(&self) -> Result<(), S::Error> {
        self.try_acquire(1)
    }

    /// Tries to acquire N or all available tokens if `available < N`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the storage could not save/load state.
    pub fn try_acquire_n_or_all(&self, permits: u32) -> Result<(), S::Error> {
        self.storage
            .try_acquire(TokenBucketAlgorithm { mode: Mode::All }, permits)
    }
}

/// Struct that implements token bucket algorithm.
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
