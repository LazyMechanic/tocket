# Tocket
This library provides implementation of token bucket algorithm.

### Example
```rust
use tocket::{TokenBucket, InMemoryStorage};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    let tb = TokenBucket::new(InMemoryStorage::new(100));
    let tb = Arc::new(tb);

    for _ in 0..8 {
        std::thread::spawn({
            let tb = Arc::clone(&tb);
            move || {
                loop {
                    match tb.try_acquire_one() {
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
    }
}
```

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>