use crate::peer_addr::PeerAddr;
use anyhow::Result;
use bytes::BytesMut;
use futures::StreamExt;
use iroh::{
    Endpoint, EndpointId,
    endpoint::{self, Connection},
};
use ring_channel::{RingReceiver, RingSender, ring_channel};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub struct Peer {
    pub addr: PeerAddr,
    endpoint: Endpoint,
    send_queue: RingSender<BytesMut>,
    route_queue: mpsc::Sender<BytesMut>,
    connect_request: mpsc::Sender<oneshot::Sender<Connection>>,
}

impl Peer {
    pub async fn handle(
        endpoint: Endpoint,
        id: EndpointId,
        route_queue: mpsc::Sender<BytesMut>,
    ) -> Arc<Peer> {
        let (send_queue, receiver) = ring_channel(1.try_into().unwrap());
        let (connect_request, connector) = mpsc::channel(1);

        let peer = Arc::new(Peer {
            addr: id.into(),
            endpoint,
            route_queue,
            send_queue,
            connect_request,
        });

        peer.clone().receiver(receiver).await;
        peer.clone().connector(connector).await;

        peer
    }

    // Sends bytes to the peer
    pub fn send(&self, bytes: BytesMut) {
        self.send_queue.send(bytes).ok();
    }

    async fn receiver(self: Arc<Self>, mut chan: RingReceiver<BytesMut>) {
        tokio::spawn(async move {
            while let Some(bytes) = chan.next().await {
                match self.get_connection().await {
                    Ok(conn) => {
                        println!("got connection!");
                        println!(
                            "{} bytes must go to peer {}",
                            bytes.len(),
                            self.addr.id.fmt_short()
                        );
                    }
                    _ => {}
                }
            }
        });
    }

    async fn get_connection(&self) -> Result<Connection> {
        let (tx, rx) = oneshot::channel();
        self.connect_request.send(tx).await?;
        Ok(rx.await?)
    }

    async fn connector(self: Arc<Self>, mut chan: mpsc::Receiver<oneshot::Sender<Connection>>) {
        tokio::spawn(async move {
            while let Some(request) = chan.recv().await {
                let conn = self.endpoint.connect(self.addr.id, b"petope/1").await;
                match conn {
                    Ok(conn) => {
                        request.send(conn).ok();
                    }
                    Err(e) => eprintln!("connect to {} failed: {:?}", self.addr.id.fmt_short(), e),
                }
            }
        });
    }
}
