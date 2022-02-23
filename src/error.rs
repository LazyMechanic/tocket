#[derive(Debug, thiserror::Error)]
#[error("rate limit exceeded")]
pub struct Error;
