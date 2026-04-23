use crate::{packet, state::State, tun, utils};
use anyhow::Result;
use bytes::BytesMut;
use etherparse::IpSlice;
use iroh::{Endpoint, EndpointId};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
};
use tokio::sync::mpsc;

pub struct Peer {
    pub id: EndpointId,
    pub addr: IpAddr,
}

pub struct Router {
    pub addr_v4: Ipv4Addr,
    pub addr_v6: Ipv6Addr,
    endpoint: Endpoint,
    route_queue: mpsc::Sender<BytesMut>,
    send_queue: mpsc::Sender<BytesMut>,
}

impl Router {
    pub async fn run(state: &State, endpoint: Endpoint) -> Result<Arc<Self>> {
        let (addr_v4, addr_v6) = utils::ip_pair_from_id(endpoint.id());

        let (route_queue, incoming) = mpsc::channel(128);
        let (send_queue, outcoming) = mpsc::channel(128);
        tun::create_tun(state, (addr_v4, addr_v6), route_queue.clone(), outcoming).await?;

        let router = Arc::new(Router {
            addr_v4,
            addr_v6,
            route_queue,
            send_queue,
            endpoint,
        });

        router.clone().receive(incoming).await;

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

        if dst == self.addr_v4 || dst == self.addr_v6 {
            self.send_queue.send(bytes).await?;
            return Ok(());
        }

        println!(
            "{:?} {} -> {} (capacity {}/{})",
            ip.payload_ip_number(),
            ip.source_addr(),
            ip.destination_addr(),
            self.route_queue.capacity(),
            self.route_queue.max_capacity()
        );

        println!("routing to peers not done yet");

        Ok(())
    }
}
