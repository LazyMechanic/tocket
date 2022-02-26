# Tocket
The library provides trait and simple implementations (in memory, redis) to limit requests with the "token bucket" algorithm.

### Example
```rust
use tocket::{RateLimiter, InMemoryTokenBucket};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    let rl = InMemoryTokenBucket::new(100);
    let rl = Arc::new(rl);
    
    let mut handles = Vec::with_capacity(8);
    for _ in 0..8 {
        let h = std::thread::spawn({
            let rl = Arc::clone(&rl);
            move || {
                loop {
                    match rl.try_acquire_one() {
                        Ok(_) => {
                            println!("token acquired, limit not exceeded");
                        }
                        Err(err) => {
                            eprintln!("token acquiring failed: {}", err);
                        }
                    }
                    
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        });
        
        handles.push(h);
    }
    
    loop {}
}
```