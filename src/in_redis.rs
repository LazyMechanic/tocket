use crate::{Error, RateLimiter};

use std::time::{Duration, SystemTime};

/// Rate limiter that implements token bucket algorithm with storage in Redis.
pub struct RedisTokenBucket {
    conn: parking_lot::Mutex<redis::Connection>,
    cap: u32,
    refill_tick: Duration,
    available_tokens_key: String,
    last_refill_key: String,
}

impl RedisTokenBucket {
    /// Creates new rate limiter with max rate limit of `rps_limit`.
    /// `available_tokens_key` and `last_refill_key` are the keys
    /// in Redis that contain these values.
    ///
    /// # Errors
    ///
    /// Will return `Err` if failed to connect to the Redis.
    pub fn new<S1, S2, S3>(
        conn_info: S1,
        rps_limit: u32,
        available_tokens_key: S2,
        last_refill_key: S3,
    ) -> Result<Self, Error>
    where
        S1: AsRef<str>,
        S2: Into<String>,
        S3: Into<String>,
    {
        let client =
            redis::Client::open(conn_info.as_ref()).map_err(|err| Error::Other(Box::new(err)))?;
        let conn = client
            .get_connection()
            .map_err(|err| Error::Other(Box::new(err)))?;

        Ok(Self {
            conn: parking_lot::Mutex::new(conn),
            cap: rps_limit,
            refill_tick: Duration::from_secs(1) / rps_limit,
            available_tokens_key: available_tokens_key.into(),
            last_refill_key: last_refill_key.into(),
        })
    }
}

impl RateLimiter for RedisTokenBucket {
    fn try_acquire(&self, permits: u32) -> Result<(), Error> {
        let mut conn = self.conn.lock();
        redis::transaction(
            &mut *conn,
            &[&self.available_tokens_key, &self.last_refill_key],
            |conn, pipe| {
                let (available_tokens, last_refill_ts): (Option<u32>, Option<u64>) = pipe
                    .get(&self.available_tokens_key)
                    .get(&self.last_refill_key)
                    .query(conn)?;

                let mut state = RedisTokenBucketState {
                    cap: self.cap,
                    available_tokens: available_tokens.unwrap_or(self.cap),
                    last_refill_ts: last_refill_ts.unwrap_or(0),
                    refill_tick: self.refill_tick,
                };
                let result = try_acquire(&mut state, permits);

                pipe.set(&self.available_tokens_key, state.available_tokens)
                    .set(&self.last_refill_key, state.last_refill_ts)
                    .query(conn)?;

                Ok(Some(result))
            },
        )
        .map_err(|err| Error::Other(Box::new(err)))?
    }
}

struct RedisTokenBucketState {
    cap: u32,
    available_tokens: u32,
    last_refill_ts: u64,
    refill_tick: Duration,
}

fn try_acquire(state: &mut RedisTokenBucketState, permits: u32) -> Result<(), Error> {
    refill(state);

    if state.available_tokens >= permits {
        state.available_tokens -= permits;
        Ok(())
    } else {
        Err(Error::RateLimitExceeded)
    }
}

fn refill(state: &mut RedisTokenBucketState) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        // Converts u128 to u64.
        // SAFETY: Safe until 2554-07-21T23:34:33.709551615+00:00
        .try_into()
        .unwrap_or(u64::MAX);

    let refill_tick_ns = state
        .refill_tick
        .as_nanos()
        // Converts u128 to u64.
        // SAFETY: Max refill tick is 1_000_000_000 as nanos that equals 1 req/sec
        .try_into()
        .unwrap_or(u64::MAX);

    let since_last_refill_ns = now - state.last_refill_ts;

    if since_last_refill_ns <= refill_tick_ns {
        return;
    }

    let tokens_since_last_refill = since_last_refill_ns / refill_tick_ns;

    state.available_tokens = u32::min(
        state.available_tokens
            + u32::try_from(tokens_since_last_refill)
                .unwrap_or_else(|_| state.cap - state.available_tokens),
        state.cap,
    );
    state.last_refill_ts += refill_tick_ns * tokens_since_last_refill;
}

#[cfg(all(test, feature = "dev-redis-impl"))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn try_acquire() {
        let rl = RedisTokenBucket::new(
            std::env::var("REDIS_HOST").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned()),
            2,
            "available_tokens",
            "last_refill",
        )
        .unwrap();

        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());
    }
}
