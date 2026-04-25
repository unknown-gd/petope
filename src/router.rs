use crate::{
    config::Config, connection_manager::ConnectionManager, peer_addr::PeerAddr,
    peer_router::PeerRouter, tun::TunDevice,
};
use anyhow::{Context, Result};
use bytes::Bytes;
use etherparse::IpSlice;
use futures::StreamExt;
use iroh::Endpoint;
use ring_channel::{RingReceiver, RingSender, ring_channel};
use std::{collections::HashMap, net::IpAddr, num::NonZeroUsize, sync::Arc};

pub struct Router {
    me: PeerAddr,
    device: TunDevice,
    manager: Arc<ConnectionManager>,
    peer_router: PeerRouter,
    from_network_rx: RingReceiver<Bytes>,
    to_network_tx: RingSender<Bytes>,
    peer_routing_tx: HashMap<IpAddr, RingSender<Bytes>>,
}

impl Router {
    pub fn new(config: &Config, endpoint: Endpoint) -> Result<Self> {
        let me = PeerAddr::from(endpoint.id());
        let (from_network_tx, from_network_rx) = ring_channel(NonZeroUsize::new(128).unwrap());
        let (to_network_tx, to_network_rx) = ring_channel::<Bytes>(NonZeroUsize::new(128).unwrap());
        let (from_peer_tx, from_peer_rx) = ring_channel(NonZeroUsize::new(128).unwrap());

        let device = TunDevice::new(config, &endpoint.id(), from_network_tx, to_network_rx)
            .context("TunDevice::new")?;

        let manager = Arc::new(ConnectionManager::new(endpoint, from_peer_tx));

        let (peer_router, peer_routing_tx) =
            PeerRouter::new(config, manager.clone(), from_peer_rx, to_network_tx.clone());

        println!("---");
        println!("current id: {}", me.id.to_z32());
        println!("ipv4: {} ipv6: {}", me.v4, me.v6);
        println!("---");
        println!("peers:");
        for p in &peer_router.peers {
            println!(" - {} ipv4: {} ipv6: {}", &p.id.to_z32()[..6], p.v4, p.v6);
        }
        println!("---");

        Ok(Router {
            me,
            device,
            manager,
            peer_router,
            from_network_rx,
            to_network_tx,
            peer_routing_tx,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        // handles tun i/o
        self.device.handle().await.context("handle tun device")?;

        // handles connections
        tokio::spawn(async move {
            self.manager.run().await;
        });

        // routes packets for individual peers
        self.peer_router.handle().await;

        tokio::spawn(async move {
            while let Some(bytes) = self.from_network_rx.next().await {
                if let Err(e) =
                    Self::route(bytes, self.me, &self.to_network_tx, &self.peer_routing_tx)
                {
                    eprintln!("routing error: {:?}", e);
                }
            }
        });

        tokio::signal::ctrl_c().await?;
        println!("bye-bye");
        Ok(())
    }

    fn route(
        bytes: Bytes,
        me: PeerAddr,
        to_network_tx: &RingSender<Bytes>,
        peer_routing_tx: &HashMap<IpAddr, RingSender<Bytes>>,
    ) -> Result<()> {
        let ip = IpSlice::from_slice(&bytes[..]).context("parse incoming ip packet")?;
        let dst = ip.destination_addr();

        if me == dst {
            let _ = to_network_tx.send(bytes);
            return Ok(());
        }

        if let Some(target) = peer_routing_tx.get(&dst) {
            let _ = target.send(bytes);
        }

        Ok(())
    }
}
