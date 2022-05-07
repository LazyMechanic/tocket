use crate::distributed::codec::Codec;
use crate::distributed::{AcquireRx, Strategy};
use crate::InMemoryStorage;

use futures::StreamExt;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio_util::udp::UdpFramed;

pub(super) async fn process<S1>(
    socket: UdpSocket,
    mut strategy: S1,
    storage: Arc<InMemoryStorage>,
    mut acq_rx: AcquireRx,
) where
    S1: Strategy,
{
    tracing::debug!("start background task");
    let mut framed = UdpFramed::new(socket, Codec::default());

    loop {
        tokio::select! {
            res = acq_rx.recv() => {
                match res {
                    Some(permits) => {
                        tracing::debug!("received acquiring of {} permits", permits);
                        if let Err(err) = strategy.on_acquire(permits, &mut framed).await {
                            tracing::error!("processing of acquiring failed: {}", err);
                        }
                    }
                    // Channel closed
                    None => break,
                }
            }
            res = framed.next() => {
                let res = res.expect("received None from udp, this is a bug");
                match res {
                    Ok((msg, addr)) => {
                        tracing::debug!("received message from peer {}: {:?}", addr, msg);
                        if let Err(err) = strategy.on_msg_recv(msg, addr, &storage, &mut framed).await {
                            tracing::error!("processing of message from peer {} failed: {}", addr, err);
                        }
                    }
                    Err(err) => {
                        tracing::error!("received error on message processing: {}", err);
                    }
                }
            }
        }
    }

    tracing::debug!("stop background task");
}
