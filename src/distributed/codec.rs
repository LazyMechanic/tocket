use crate::distributed::message::Message;
use crate::error::DistributedStorageError;

use bytes::{BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Default)]
pub struct Codec(());

impl Encoder<Message> for Codec {
    type Error = DistributedStorageError;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let enc = bincode::serialize(&item)?;
        dst.reserve(enc.len());
        dst.put_slice(&enc);
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = Message;
    type Error = DistributedStorageError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let len = src.len();
        let buf = src.split_to(len);
        let item: Message = bincode::deserialize(&buf)?;
        item.check_checksum()?;
        Ok(Some(item))
    }
}
