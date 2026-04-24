use crate::{
    config::{Config, Peer},
    connection_manager::ConnectionManager,
    peer_addr::PeerAddr,
    tun::{self, TunDevice},
};
use anyhow::{Context, Result};
use bytes::{Bytes, BytesMut};
use etherparse::IpSlice;
use iroh::{Endpoint, EndpointId};
use ring_channel::ring_channel;
use std::{collections::HashMap, net::IpAddr, num::NonZeroUsize, sync::Arc};
use tokio::sync::mpsc;

pub struct Router {
    device: TunDevice,
}

impl Router {
    pub fn new(config: &Config, endpoint: Endpoint) -> Result<Self> {
        let (from_network_tx, from_network_rx) = ring_channel(NonZeroUsize::new(8).unwrap());
        let (to_network_tx, to_network_rx) = ring_channel::<Bytes>(NonZeroUsize::new(8).unwrap());

        let device = TunDevice::new(config, &endpoint.id(), from_network_tx, to_network_rx)
            .context("TunDevice::new")?;

        Ok(Router { device })
    }

    pub async fn run(self) -> Result<()> {
        tokio::spawn(async move {
            if let Err(e) = self.device.run().await {
                eprintln!("tun device fatal error: {:?}", e);
            }
        });

        tokio::signal::ctrl_c().await?;
        println!("bye-bye");
        Ok(())
    }
}
