use crate::distributed::codec::Codec;
use crate::distributed::message::{Content, ContentKind, Message, WhitelistContent};
use crate::error::DistributedStorageError;
use crate::{InMemoryStorage, Mode, Storage, Strategy, TokenBucketAlgorithm};

use futures::SinkExt;
use std::collections::HashSet;
use std::net::{SocketAddr, ToSocketAddrs};
use tokio_util::udp::UdpFramed;

const MAX_TS_DIFF: time::Duration = time::Duration::seconds(5);

pub struct WhitelistStrategy {
    peers: HashSet<SocketAddr>,
}

impl WhitelistStrategy {
    pub fn new<I, S>(peers: I) -> Result<Self, DistributedStorageError>
    where
        I: IntoIterator<Item = S>,
        S: ToSocketAddrs,
    {
        let peers = peers
            .into_iter()
            .map(|p| p.to_socket_addrs().map_err(DistributedStorageError::from))
            .map(|v| match v {
                Ok(mut addrs) => addrs
                    .next()
                    .ok_or_else(|| DistributedStorageError::PeerAddrNotResolved),
                Err(err) => Err(err),
            })
            .collect::<Result<HashSet<_>, _>>()?;

        Ok(Self { peers })
    }
}

#[async_trait::async_trait]
impl Strategy for WhitelistStrategy {
    async fn on_acquire(
        &mut self,
        permits: u32,
        framed: &mut UdpFramed<Codec>,
    ) -> Result<(), DistributedStorageError> {
        let msg = Message::new(Content::Whitelist(WhitelistContent {
            sent_ts: time::OffsetDateTime::now_utc(),
            permits,
        }));

        for peer in &self.peers {
            framed.send((msg.clone(), *peer)).await?
        }

        Ok(())
    }

    async fn on_msg_recv(
        &mut self,
        msg: Message,
        source: SocketAddr,
        storage: &InMemoryStorage,
        _framed: &mut UdpFramed<Codec>,
    ) -> Result<(), DistributedStorageError> {
        if !self.peers.contains(&source) {
            return Err(DistributedStorageError::PeerNotWhitelisted { peer: source });
        }

        match msg.content {
            Content::Whitelist(content) => {
                let now = time::OffsetDateTime::now_utc();
                if content.sent_ts < now - MAX_TS_DIFF || content.sent_ts > now {
                    tracing::warn!("received expired message, skip it");
                    return Ok(());
                }

                storage.try_acquire(TokenBucketAlgorithm { mode: Mode::All }, content.permits)?;
                Ok(())
            }
            x => Err(DistributedStorageError::MessageContentMismatch {
                exp: ContentKind::Whitelist,
                act: x.kind(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DistributedStorage, TokenBucket};
    use std::time::Duration;

    #[tokio::test]
    async fn t1() {
        let strategy = WhitelistStrategy::new(Vec::<String>::new()).unwrap();
        let storage = DistributedStorage::serve(2, "0.0.0.0:0", strategy)
            .await
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
