use crate::{
    config::Config, connection_manager::ConnectionManager, peer::Peer, peer_addr::PeerAddr, tun,
};
use anyhow::{Context, Result};
use bytes::BytesMut;
use etherparse::IpSlice;
use iroh::{Endpoint, EndpointId};
use std::{collections::HashMap, net::IpAddr, sync::Arc};
use tokio::sync::mpsc;

pub struct Router {
    pub me: PeerAddr,
    pub peers: HashMap<EndpointId, Arc<Peer>>,

    route_queue: mpsc::Sender<BytesMut>,
    send_queue: mpsc::Sender<BytesMut>,
    peer_routing_table: HashMap<IpAddr, Arc<Peer>>,
    manager: ConnectionManager,
}

impl Router {
    pub async fn run(config: &Config, endpoint: Endpoint) -> Result<Arc<Self>> {
        let me: PeerAddr = endpoint.id().into();

        let manager = ConnectionManager::new(endpoint.clone());

        let (route_queue, incoming) = mpsc::channel(128);
        let (send_queue, outcoming) = mpsc::channel(128);
        let ifindex =
            tun::create_tun(config, (me.v4, me.v6), route_queue.clone(), outcoming).await?;

        let mut peers = HashMap::with_capacity(config.peers.len());
        for c in &config.peers {
            let p = Peer::handle(endpoint.clone(), c.id, route_queue.clone()).await;
            peers.insert(p.addr.id, p);
        }

        Router::setup_routes(peers.values().map(|p| &p.addr), ifindex)
            .await
            .context("setup routes")?;

        let mut peer_routing_table = HashMap::new();
        for peer in peers.values() {
            peer_routing_table.insert(peer.addr.v4.into(), peer.clone());
            peer_routing_table.insert(peer.addr.v6.into(), peer.clone());
        }

        let router = Arc::new(Router {
            me,
            peers,
            peer_routing_table,
            route_queue,
            send_queue,
            manager,
        });

        router.clone().receive(incoming).await;
        router.clone().acceptor(endpoint).await;
        router.clone().manage_connections().await;

        Ok(router)
    }

    // runs on background a worker that receives bytes and routes them
    async fn receive(self: Arc<Self>, mut receiver: mpsc::Receiver<BytesMut>) {
        tokio::spawn(async move {
            while let Some(bytes) = receiver.recv().await {
                self.route(bytes)
                    .await
                    .unwrap_or_else(|e| eprintln!("unable to route a packet: {:?}", e));
            }
        });
    }

    async fn route(&self, bytes: BytesMut) -> Result<()> {
        let ip = IpSlice::from_slice(&bytes)?;
        let dst = ip.destination_addr();

        if self.route_queue.capacity() < 4 {
            println!(
                "too much incoming, channel capacity {}/{}",
                self.route_queue.capacity(),
                self.route_queue.max_capacity()
            );
        }

        if self.me == dst {
            self.send_queue.send(bytes).await?;
            return Ok(());
        }

        if let Some(peer) = self.peer_routing_table.get(&dst) {
            if let Some(conn) = self.manager.get(peer.addr.id).await {
                if let Err(e) = conn.send_datagram(bytes.into()) {
                    println!(
                        "err while sending data to {}: {:?}",
                        peer.addr.id.fmt_short(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    async fn setup_routes(peers: impl Iterator<Item = &PeerAddr>, ifindex: u32) -> Result<()> {
        let handle = net_route::Handle::new()?;
        for p in peers {
            p.add_route(&handle, ifindex)
                .await
                .with_context(|| format!("add route for {} peer", p.id.fmt_short()))?;
        }

        Ok(())
    }

    async fn acceptor(self: Arc<Self>, ep: Endpoint) {
        tokio::spawn(async move {
            while let Some(incoming) = ep.accept().await {
                // TODO: ip filtering here
                let remote_addr = incoming.remote_addr();
                match incoming.await {
                    Ok(conn) => {
                        let peer_id = conn.remote_id();
                        if let Some(peer) = self.peers.get(&peer_id) {
                            peer.accept(conn).await;
                        }
                    }
                    Err(e) => eprintln!(
                        "unable to accept connection from {:?}: {:?}",
                        remote_addr, e
                    ),
                }
            }
        });
    }

    async fn manage_connections(self: Arc<Self>) {
        tokio::spawn(async move {
            self.manager.run().await;
        });
    }
}
