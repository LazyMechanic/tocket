use crate::in_memory::InMemoryTokenBucketInner;
use crate::Error;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{ToSocketAddrs, UdpSocket};
use tokio_util::sync::CancellationToken;

pub struct DistributedTokenBucket {
    state: SharedState,
    cancel_tok: CancellationToken,
}

type SharedState = Arc<parking_lot::Mutex<InMemoryTokenBucketInner>>;

impl DistributedTokenBucket {
    pub async fn serve<L, E, A>(
        rps_limit: u32,
        listen: L,
        endpoints: E,
    ) -> Result<DistributedTokenBucket, Error>
    where
        L: ToSocketAddrs,
        E: IntoIterator<Item = A> + Send + 'static,
        A: ToSocketAddrs + 'static,
    {
        let socket = UdpSocket::bind(listen)
            .await
            .map_err(|err| Error::Other(Box::new(err)))?;

        let cancel_tok = CancellationToken::new();

        let state = Arc::new(parking_lot::Mutex::new(InMemoryTokenBucketInner {
            cap: rps_limit,
            available_tokens: rps_limit,
            last_refill: Instant::now(),
            refill_tick: Duration::from_secs(1) / rps_limit,
        }));

        tokio::spawn(process(
            socket,
            cancel_tok.child_token(),
            Arc::clone(&state),
            endpoints,
        ));

        Ok(Self { state, cancel_tok })
    }
}

impl Drop for DistributedTokenBucket {
    fn drop(&mut self) {
        self.cancel_tok.cancel();
    }
}

async fn process<E, A>(
    socket: UdpSocket,
    cancel_tok: CancellationToken,
    state: SharedState,
    endpoints: E,
) where
    E: IntoIterator<Item = A>,
    A: ToSocketAddrs,
{
}

#[derive(Debug, Clone, Eq, PartialEq, borsh::BorshSerialize, borsh::BorshDeserialize)]
struct Message {
    init_timestamp: i64,
    seqno: u64,
    available_tokens: u32,
    checksum: u32,
}
