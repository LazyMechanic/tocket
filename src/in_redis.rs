use crate::{Error, RateLimiter};

use std::time::{Duration, SystemTime};

pub struct RedisTokenBucket {
    conn: parking_lot::Mutex<redis::Connection>,
    cap: u32,
    refill_tick: Duration,
    available_tokens_key: String,
    last_refill_key: String,
}

impl RedisTokenBucket {
    pub fn new<S1, S2>(
        conn: redis::Connection,
        rps: u32,
        available_tokens_key: S1,
        last_refill_key: S2,
    ) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Self {
            conn: parking_lot::Mutex::new(conn),
            cap: rps,
            refill_tick: Duration::from_secs(1) / rps,
            available_tokens_key: available_tokens_key.into(),
            last_refill_key: last_refill_key.into(),
        }
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
        .as_nanos() as u64;

    let refill_tick_ns = state.refill_tick.as_nanos() as u64;
    let since_last_refill_ns = now - state.last_refill_ts;

    if since_last_refill_ns <= refill_tick_ns {
        return;
    }

    let tokens_since_last_refill = since_last_refill_ns / refill_tick_ns;

    state.available_tokens = u32::min(
        state.available_tokens + tokens_since_last_refill as u32,
        state.cap,
    );
    state.last_refill_ts += refill_tick_ns * tokens_since_last_refill;
}

#[cfg(all(test, feature = "test-redis"))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn try_acquire() {
        let host =
            std::env::var("REDIS_HOST").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_owned());
        let client = redis::Client::open(host).unwrap();

        let rl = RedisTokenBucket::new(
            client.get_connection().unwrap(),
            2,
            "available_tokens",
            "last_refill",
        );

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
