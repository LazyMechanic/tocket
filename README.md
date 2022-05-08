# Tocket

[<img alt="crates.io" src="https://img.shields.io/crates/v/tocket?style=flat-square">](https://crates.io/crates/tocket)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/tocket?style=flat-square">](https://docs.rs/tocket)
[<img alt="build" src="https://img.shields.io/github/workflow/status/LazyMechanic/tocket/Rust?style=flat-square">](https://github.com/LazyMechanic/tocket/actions)

This library provides implementation of token bucket algorithm and some storage implementations.

```toml
[dependencies]
tocket = "0.2"
```

### Example
```rust
use tocket::{TokenBucket, InMemoryStorage};
use std::sync::Arc;
use std::time::Duration;

fn main() {
    let tb = TokenBucket::new(InMemoryStorage::new(100));
    let tb = Arc::new(tb);

    for i in 0..4 {
        std::thread::spawn({
            let tb = Arc::clone(&tb);
            move || {
                loop {
                    std::thread::sleep(Duration::from_secs(i));
                    
                    match tb.try_acquire_one() {
                        Ok(_) => println!("token acquired, limit not exceeded"),
                        Err(err) => eprintln!("token acquiring failed: {}", err),
                    }
                }
            }
        });
    }
    
    loop {}
}
```

## Features
- `redis-impl` - redis storage implementation
- `distributed-impl` - distributed storage implementation

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