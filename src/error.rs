#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("rate limit exceeded")]
    RateLimitExceeded,

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync + 'static>),
}
