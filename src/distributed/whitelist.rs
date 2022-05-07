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
            framed.send((msg.clone(), *peer)).await?;
            tracing::debug!("sent message to peer {}: {:?}", peer, msg);
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

        // TODO: remove allowing when add another one strategy
        #[allow(unreachable_patterns)]
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

    async fn make_token_bucket<I, S>(port: u16, peers: I) -> TokenBucket<DistributedStorage>
    where
        I: IntoIterator<Item = S>,
        S: ToSocketAddrs,
    {
        let storage = DistributedStorage::serve(
            2,
            format!("0.0.0.0:{}", port),
            WhitelistStrategy::new(peers).unwrap(),
        )
        .await
        .unwrap();

        TokenBucket::new(storage)
    }

    #[tokio::test]
    async fn try_acquire_single() {
        let tb = make_token_bucket(0, Vec::<String>::new()).await;

        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());

        std::thread::sleep(Duration::from_millis(1500));
        assert!(tb.try_acquire(2).is_ok());
        assert!(tb.try_acquire_one().is_err());
    }

    #[tokio::test]
    async fn try_acquire_multiple() {
        let tb1 = make_token_bucket(49001, vec!["127.0.0.1:49002", "127.0.0.1:49003"]).await;
        let tb2 = make_token_bucket(49002, vec!["127.0.0.1:49001", "127.0.0.1:49003"]).await;
        let tb3 = make_token_bucket(49003, vec!["127.0.0.1:49001", "127.0.0.1:49002"]).await;

        assert!(tb1.try_acquire(2).is_ok());
        assert!(tb1.try_acquire_one().is_err());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(tb2.try_acquire_one().is_err());
        assert!(tb3.try_acquire_one().is_err());

        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(tb1.try_acquire(2).is_ok());
        assert!(tb1.try_acquire_one().is_err());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(tb2.try_acquire_one().is_err());
        assert!(tb3.try_acquire_one().is_err());

        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(tb1.try_acquire(2).is_ok());
        assert!(tb1.try_acquire_one().is_err());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(tb2.try_acquire_one().is_err());
        assert!(tb3.try_acquire_one().is_err());
    }
}
