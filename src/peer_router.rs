use crate::{config::Config, connection_manager::ConnectionManager, peer_addr::PeerAddr, utils};
use bytes::Bytes;
use futures::StreamExt;
use iroh::EndpointId;
use ring_channel::{RingReceiver, RingSender, ring_channel};
use std::{collections::HashMap, net::IpAddr, num::NonZeroUsize, sync::Arc};

pub struct PeerRouter {
    pub peers: Vec<PeerAddr>,
    from_peer_rx: RingReceiver<Bytes>,
    peer_routing_rx: HashMap<EndpointId, RingReceiver<Bytes>>,
    to_network_tx: RingSender<Bytes>,
    manager: Arc<ConnectionManager>,
}

impl PeerRouter {
    pub fn new(
        config: &Config,
        manager: Arc<ConnectionManager>,
        from_peer_rx: RingReceiver<Bytes>,
        to_network_tx: RingSender<Bytes>,
    ) -> (Self, HashMap<IpAddr, RingSender<Bytes>>) {
        let peers: Vec<PeerAddr> = config.peers.iter().map(|c| PeerAddr::from(c.id)).collect();

        let mut peer_routing_rx = HashMap::new();
        let mut peer_routing_tx = HashMap::new();
        for p in &peers {
            let (tx, rx) = ring_channel(NonZeroUsize::new(128).unwrap());
            peer_routing_rx.insert(p.id, rx);
            peer_routing_tx.insert(IpAddr::V4(p.v4), tx.clone());
            peer_routing_tx.insert(IpAddr::V6(p.v6), tx);
        }

        let router = PeerRouter {
            peers,
            manager,
            peer_routing_rx,
            from_peer_rx,
            to_network_tx,
        };

        (router, peer_routing_tx)
    }

    pub async fn handle(mut self) {
        // sends data from the router to the datagram
        for (id, mut rx) in self.peer_routing_rx {
            let manager = self.manager.clone();
            tokio::spawn(async move {
                while let Some(bytes) = rx.next().await {
                    if let Some(conn) = manager.get(id).await {
                        if let Err(e) = conn.send_datagram(bytes) {
                            eprintln!("send to {} failed: {:?}", utils::short_id(&id), e);
                        }
                    }
                }
            });
        }

        // receives data from connection manager, validates it and forwards to the router
        tokio::spawn(async move {
            while let Some(bytes) = self.from_peer_rx.next().await {
                let _ = self.to_network_tx.send(bytes);
            }
        });
    }
}
