use crate::error::DistributedStorageError;

use borsh::{BorshDeserialize, BorshSerialize};
use std::borrow::Cow;
use std::hash::Hash;
use std::io::Write;

#[derive(Debug, Clone, Eq, PartialEq, BorshSerialize, BorshDeserialize)]
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
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, BorshSerialize, BorshDeserialize)]
pub enum Content {
    Whitelist(WhitelistContent),
}

impl Content {
    pub fn kind(&self) -> ContentKind {
        match self {
            Content::Whitelist(_) => ContentKind::Whitelist,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WhitelistContent {
    pub sent_ts: time::OffsetDateTime,
    pub permits: u32,
}

impl BorshSerialize for WhitelistContent {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        BorshSerialize::serialize(&self.sent_ts.year(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.ordinal(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.hour(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.minute(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.second(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.nanosecond(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.offset().whole_hours(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.offset().minutes_past_hour(), writer)?;
        BorshSerialize::serialize(&self.sent_ts.offset().seconds_past_minute(), writer)?;
        BorshSerialize::serialize(&self.permits, writer)?;

        Ok(())
    }
}

impl BorshDeserialize for WhitelistContent {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let year = <i32 as BorshDeserialize>::deserialize(buf)?;
        let ordinal = <u16 as BorshDeserialize>::deserialize(buf)?;
        let hour = <u8 as BorshDeserialize>::deserialize(buf)?;
        let minute = <u8 as BorshDeserialize>::deserialize(buf)?;
        let second = <u8 as BorshDeserialize>::deserialize(buf)?;
        let nanosecond = <u32 as BorshDeserialize>::deserialize(buf)?;
        let offset_hours = <i8 as BorshDeserialize>::deserialize(buf)?;
        let offset_minutes = <i8 as BorshDeserialize>::deserialize(buf)?;
        let offset_seconds = <i8 as BorshDeserialize>::deserialize(buf)?;

        let sent_ts = time::Date::from_ordinal_date(year, ordinal)
            .and_then(|date| date.with_hms_nano(hour, minute, second, nanosecond))
            .and_then(|datetime| {
                time::UtcOffset::from_hms(offset_hours, offset_minutes, offset_seconds)
                    .map(|offset| datetime.assume_offset(offset))
            })
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
        let permits = <u32 as BorshDeserialize>::deserialize(buf)?;

        Ok(Self { sent_ts, permits })
    }
}
