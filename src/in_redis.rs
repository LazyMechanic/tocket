use crate::{RateLimitExceededError, State, Storage, TokenBucketAlgorithm};

/// Default key of available tokens in redis
pub const AVAILABLE_TOKENS_KEY: &str = "tocket::available_tokens";
/// Default key of last refill in redis
pub const LAST_REFILL_KEY: &str = "tocket::last_refill";

/// A storage that stores state in Redis.
///
/// Useful when you have multiple application instances with shared state
/// and Redis already running.
///
/// # Example
/// ```
/// # fn main() {
/// use tocket::{TokenBucket, RedisStorage};
///
/// fn main() {
///      let storage = RedisStorage::new(2, "redis://127.0.0.1:6379").unwrap();
///
///     let tb = TokenBucket::new(storage);
///     assert!(tb.try_acquire(2).is_ok());
///     assert!(tb.try_acquire_one().is_err());
/// }
/// # }
/// ```
pub struct RedisStorage {
    conn: parking_lot::Mutex<redis::Connection>,
    cap: u32,
    refill_tick: time::Duration,
    available_tokens_key: String,
    last_refill_key: String,
}

impl RedisStorage {
    /// Creates a storage.
    ///
    /// # Errors
    ///
    /// Will return `Err` if failed to connect to the Redis.
    pub fn new<I>(rps_limit: u32, conn_info: I) -> Result<Self, RedisStorageError>
    where
        I: AsRef<str>,
    {
        let client = redis::Client::open(conn_info.as_ref())?;
        let conn = client.get_connection()?;

        Ok(Self {
            conn: parking_lot::Mutex::new(conn),
            cap: rps_limit,
            refill_tick: time::Duration::seconds(1) / rps_limit,
            available_tokens_key: AVAILABLE_TOKENS_KEY.to_owned(),
            last_refill_key: LAST_REFILL_KEY.to_owned(),
        })
    }

    /// Creates a builder of storage. Needs for customizing of redis keys
    pub fn builder<I>(rps_limit: u32, conn_info: I) -> RedisStorageBuilder
    where
        I: AsRef<str>,
    {
        RedisStorageBuilder {
            storage: Self::new(rps_limit, conn_info),
        }
    }
}

pub struct RedisStorageBuilder {
    storage: Result<RedisStorage, RedisStorageError>,
}

impl RedisStorageBuilder {
    /// Customize key for value in redis.
    pub fn with_available_tokens_key<K>(mut self, key: K) -> Self
    where
        K: Into<String>,
    {
        if let Ok(storage) = &mut self.storage {
            storage.available_tokens_key = key.into();
        }
        self
    }

    /// Customize key for value in redis.
    pub fn with_last_refill_key<K>(mut self, key: K) -> Self
    where
        K: Into<String>,
    {
        if let Ok(storage) = &mut self.storage {
            storage.last_refill_key = key.into();
        }
        self
    }

    pub fn build(self) -> Result<RedisStorage, RedisStorageError> {
        self.storage
    }
}

impl Storage for RedisStorage {
    type Error = RedisStorageError;

    fn try_acquire(&self, alg: TokenBucketAlgorithm, permits: u32) -> Result<(), Self::Error> {
        let mut conn = self.conn.lock();
        redis::transaction(
            &mut *conn,
            &[&self.available_tokens_key, &self.last_refill_key],
            move |conn, pipe| {
                let (available_tokens, last_refill_ts): (Option<u32>, Option<Vec<u8>>) = pipe
                    .get(&self.available_tokens_key)
                    .get(&self.last_refill_key)
                    .query(conn)?;

                const I128_SIZE: usize = std::mem::size_of::<i128>();

                let last_refill = match last_refill_ts {
                    Some(last_refill_ts) => {
                        let last_refill_ts_arr: [u8; I128_SIZE] = match last_refill_ts.try_into() {
                            Ok(v) => v,
                            Err(v) => {
                                return Ok(Some(Err(
                                    RedisStorageError::ConvertingBytesToI128Error {
                                        key: self.last_refill_key.clone(),
                                        value: v,
                                    },
                                )))
                            }
                        };

                        let nanos_ts = i128::from_le_bytes(last_refill_ts_arr);
                        match time::OffsetDateTime::from_unix_timestamp_nanos(nanos_ts)
                            .map_err(RedisStorageError::from)
                        {
                            Ok(v) => v,
                            Err(err) => return Ok(Some(Err(err))),
                        }
                    }
                    None => time::OffsetDateTime::now_utc(),
                };

                let mut state = State {
                    cap: self.cap,
                    available_tokens: available_tokens.unwrap_or(self.cap),
                    refill_tick: self.refill_tick,
                    last_refill,
                };
                let result = alg
                    .try_acquire(&mut state, permits)
                    .map_err(RedisStorageError::from);

                let last_refill_ts = state.last_refill.unix_timestamp_nanos().to_le_bytes();

                pipe.set(&self.available_tokens_key, state.available_tokens)
                    .set(&self.last_refill_key, &last_refill_ts)
                    .query(conn)?;

                Ok(Some(result))
            },
        )?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RedisStorageError {
    #[error(transparent)]
    RedisError(#[from] redis::RedisError),
    #[error(transparent)]
    TimeComponentRangeError(#[from] time::error::ComponentRange),
    #[error(transparent)]
    RateLimitExceededError(#[from] RateLimitExceededError),
    #[error("converting '{key}' ({value:?}) to i128 failed")]
    ConvertingBytesToI128Error { key: String, value: Vec<u8> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenBucket;

    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn try_acquire() {
        let storage = RedisStorage::builder(
            2,
            std::env::var("REDIS_HOST").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned()),
        )
        .with_last_refill_key(format!("last_refill_{}", Uuid::new_v4()))
        .with_available_tokens_key(format!("available_tokens{}", Uuid::new_v4()))
        .build()
        .unwrap();

        let tb = TokenBucket::new(storage);

        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());
    }
}
