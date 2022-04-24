use crate::error::DistributedStorageError;

use std::borrow::Cow;
use std::hash::Hash;

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub version: Cow<'static, str>,
    pub content: Content,
    pub checksum: u32,
}

impl Message {
    pub fn new(content: Content) -> Self {
        let version: Cow<'static, str> = std::env!("CARGO_PKG_VERSION").into();
        let checksum = calculate_checksum(&version, &content);

        Self {
            version,
            content,
            checksum,
        }
    }

    pub fn check_checksum(&self) -> Result<(), DistributedStorageError> {
        let calc_checksum = calculate_checksum(&self.version, &self.content);
        if calc_checksum != self.checksum {
            return Err(DistributedStorageError::ChecksumMismatch {
                act: self.checksum,
                exp: calc_checksum,
            });
        }

        Ok(())
    }
}

fn calculate_checksum(version: &str, content: &Content) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    version.hash(&mut hasher);
    content.hash(&mut hasher);
    hasher.finalize()
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ContentKind {
    Whitelist,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum Content {
    Whitelist(WhitelistContent),
    #[serde(other)]
    Unknown,
}

impl Content {
    pub fn kind(&self) -> ContentKind {
        match self {
            Content::Whitelist(_) => ContentKind::Whitelist,
            Content::Unknown => ContentKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct WhitelistContent {
    pub sent_ts: time::OffsetDateTime,
    pub permits: u32,
}
