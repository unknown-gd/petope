use crate::{packet, state::State, tun};
use anyhow::Result;
use bytes::BytesMut;
use etherparse::IpSlice;
use iroh::EndpointId;
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
    route_queue: mpsc::Sender<BytesMut>,
    send_queue: mpsc::Sender<BytesMut>,
}

impl Router {
    pub async fn run(state: &State) -> Result<Arc<Self>> {
        let (route_queue, incoming) = mpsc::channel(128);
        let (send_queue, outcoming) = mpsc::channel(128);
        tun::create_tun(state, route_queue.clone(), outcoming).await?;

        let router = Arc::new(Router {
            route_queue,
            send_queue,
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
        let me: Ipv6Addr = "fdee::1".parse().unwrap();
        let me2: Ipv4Addr = "10.1.1.1".parse().unwrap();

        if self.route_queue.capacity() < 4 {
            println!(
                "too much incoming, channel capacity {}/{}",
                self.route_queue.capacity(),
                self.route_queue.max_capacity()
            );
        }

        if ip.destination_addr() == me || ip.destination_addr() == me2 {
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
