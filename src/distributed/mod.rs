pub mod error;
pub mod whitelist;

mod codec;
mod message;
mod processing;

pub use error::DistributedStorageError;
pub use whitelist::WhitelistStrategy;

use crate::distributed::codec::Codec;
use crate::distributed::message::Message;
use crate::{InMemoryStorage, Storage, TokenBucketAlgorithm};

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_util::udp::UdpFramed;
use tracing::Instrument;

type AcquireTx = mpsc::UnboundedSender<u32>;
type AcquireRx = mpsc::UnboundedReceiver<u32>;

/// A distributed storage that under the hood stores the state in the local `InMemoryStorage`
/// and sends messages to the rest of the distributed storages via UDP messages on each tokens acquiring,
/// according to the strategy used.
///
/// Useful when you have multiple application instances with shared state
/// but don't want to run additional storage (e.g. Redis).
///
/// # Available strategies:
/// - [`WhitelistStrategy`]
///
/// # Example
/// See usage examples in strategies above.
///
/// [`WhitelistStrategy`]: crate::distributed::whitelist::WhitelistStrategy
pub struct DistributedStorage {
    tx: AcquireTx,
    storage: Arc<InMemoryStorage>,
    listen_addr: SocketAddr,
}

impl DistributedStorage {
    /// Creates a distributed storage with the given strategy
    /// and starts a background task that will listen a UDP socket.
    ///
    /// # Errors
    ///
    /// Will return `Err` if failed to resolve listen address.
    pub async fn serve<A, S>(
        rps_limit: u32,
        listen_addr: A,
        strategy: S,
    ) -> Result<Self, DistributedStorageError>
    where
        A: ToSocketAddrs,
        S: Strategy + Send + 'static,
    {
        let listen_addr = listen_addr.to_socket_addrs()?.collect::<Vec<_>>();
        let socket = UdpSocket::bind(listen_addr.as_slice()).await?;
        let listen_addr = socket.local_addr()?;

        let storage = Arc::new(InMemoryStorage::new(rps_limit));
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(
            processing::process(socket, strategy, Arc::clone(&storage), rx)
                .instrument(tracing::Span::current()),
        );

        Ok(Self {
            tx,
            storage,
            listen_addr,
        })
    }

    /// Get listen address.
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }
}

impl Storage for DistributedStorage {
    type Error = DistributedStorageError;

    fn try_acquire(&self, alg: TokenBucketAlgorithm, permits: u32) -> Result<(), Self::Error> {
        self.storage.try_acquire(alg, permits)?;
        self.tx
            .send(permits)
            .expect("sending permits to background task failed, this is a bug");
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Strategy: private::Sealed {
    async fn on_acquire(
        &mut self,
        permits: u32,
        framed: &mut UdpFramed<Codec>,
    ) -> Result<(), DistributedStorageError>;

    async fn on_msg_recv(
        &mut self,
        msg: Message,
        source: SocketAddr,
        storage: &InMemoryStorage,
        framed: &mut UdpFramed<Codec>,
    ) -> Result<(), DistributedStorageError>;
}

mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for WhitelistStrategy {}
}
