use crate::{Error, RateLimiter};

use std::time::{Duration, Instant};

pub struct InMemoryTokenBucket {
    inner: parking_lot::Mutex<InMemoryTokenBucketInner>,
}

struct InMemoryTokenBucketInner {
    cap: u32,
    available_tokens: u32,
    last_refill: Instant,
    refill_tick: Duration,
}

impl InMemoryTokenBucket {
    pub fn new(rps: u32) -> Self {
        Self {
            inner: parking_lot::Mutex::new(InMemoryTokenBucketInner {
                cap: rps,
                available_tokens: rps,
                last_refill: Instant::now(),
                refill_tick: Duration::from_secs(1) / rps,
            }),
        }
    }
}

impl RateLimiter for InMemoryTokenBucket {
    fn try_acquire(&self, permits: u32) -> Result<(), Error> {
        let mut inner = self.inner.lock();
        inner.try_acquire(permits)
    }
}

impl InMemoryTokenBucketInner {
    fn try_acquire(&mut self, permits: u32) -> Result<(), Error> {
        self.refill();

        if self.available_tokens >= permits {
            self.available_tokens -= permits;
            Ok(())
        } else {
            Err(Error)
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let since_last_refill = now - self.last_refill;

        if since_last_refill <= self.refill_tick {
            return;
        }

        let tokens_since_last_refill = {
            let mut tokens_count = 0u32;
            let mut k = since_last_refill;
            loop {
                match k.checked_sub(self.refill_tick) {
                    None => {
                        break;
                    }
                    Some(new_k) => {
                        k = new_k;
                        tokens_count += 1;
                    }
                }
            }
            tokens_count
        };

        self.available_tokens =
            u32::min(self.available_tokens + tokens_since_last_refill, self.cap);
        self.last_refill = self.last_refill + self.refill_tick * tokens_since_last_refill;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_acquire() {
        let rl = InMemoryTokenBucket::new(2);

        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_secs(1));
        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_secs(1));
        assert!(rl.try_acquire(2).is_ok());
        assert!(rl.try_acquire_one().is_err());
    }
}
