use crate::distributed::message::ContentKind;
use crate::RateLimitExceededError;
use std::net::SocketAddr;

#[derive(Debug, thiserror::Error)]
pub enum DistributedStorageError {
    #[error(transparent)]
    RateLimitExceededError(#[from] RateLimitExceededError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Bincode(#[from] bincode::Error),

    #[error("checksum does not match: actual = {act:#x} expected = {exp:#x}")]
    ChecksumMismatch { act: u32, exp: u32 },
    #[error("peer {peer} not whitelisted")]
    PeerNotWhitelisted { peer: SocketAddr },
    #[error("message content mismatch: expected '{exp:?}', but actual is '{act:?}'")]
    MessageContentMismatch { exp: ContentKind, act: ContentKind },
    #[error("peer address not resolved")]
    PeerAddrNotResolved,
}
