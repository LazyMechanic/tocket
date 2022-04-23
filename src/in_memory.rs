use crate::{RateLimitExceededError, State, Storage, TokenBucketAlgorithm};

pub struct InMemoryStorage {
    state: parking_lot::Mutex<State>,
}

impl InMemoryStorage {
    pub fn new(rps_limit: u32) -> Self {
        Self {
            state: parking_lot::Mutex::new(State {
                cap: rps_limit,
                available_tokens: rps_limit,
                last_refill: time::OffsetDateTime::now_utc(),
                refill_tick: time::Duration::seconds(1) / rps_limit,
            }),
        }
    }
}

impl Storage for InMemoryStorage {
    type Error = RateLimitExceededError;

    fn try_acquire(&self, alg: TokenBucketAlgorithm, permits: u32) -> Result<(), Self::Error> {
        let mut state = self.state.lock();
        alg.try_acquire(&mut state, permits)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokenBucket;

    use std::time::Duration;

    #[test]
    fn try_acquire() {
        let tb = TokenBucket::new(InMemoryStorage::new(2));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_secs(1));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_secs(1));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());
    }
}
