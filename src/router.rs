use crate::{packet, state::State, tun};
use anyhow::Result;
use bytes::BytesMut;
use etherparse::IpSlice;
use iroh::EndpointId;
use std::{
    net::{IpAddr, Ipv6Addr},
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
        let (route_queue, incoming) = mpsc::channel(1);
        let (send_queue, outcoming) = mpsc::channel(1);
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

        println!(
            "{:?} {} -> {}",
            ip.payload_ip_number(),
            ip.source_addr(),
            ip.destination_addr(),
        );

        if ip.destination_addr() == me {
            // special case for pinging
            if let Some(data) = packet::echo_reply(&ip) {
                println!("echo reply");
                // in most cases ping reply will be routed to a peer
                // in worst case it will be double serialized if routed to local
                // try_send since if a channel is full we may be stuck here
                self.route_queue.try_send(data).ok();
                return Ok(());
            }

            self.send_queue.send(packet::serialize_ip_slice(ip)).await?;
            return Ok(());
        }

        println!("routing to peers not done yet");

        Ok(())
    }
}
