use anyhow::Result;
use etherparse::{Icmpv4Slice, Icmpv4Type, IpNumber, NetSlice, PacketBuilder, SlicedPacket};
use net_route::{Handle, Route};
use tun_rs::DeviceBuilder;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tokio::spawn(async {
        tokio::signal::ctrl_c().await.unwrap();
        println!("bye bye");
        std::process::exit(0);
    });

    let device = DeviceBuilder::new()
        .name("utun9")
        .mtu(1000)
        .ipv4("10.0.0.100", 32, None)
        .layer(tun_rs::Layer::L3)
        .build_async()?;

    let handle = Handle::new()?;
    let route = Route::new("10.0.0.101".parse()?, 32).with_ifindex(device.if_index()?);
    handle.add(&route).await.expect("add route");

    let mut buf = vec![0; 2048];
    loop {
        let recv = device.recv(&mut buf).await?;
        let buf = &buf[..recv];

        println!("{} -> {}", recv, device.send(buf).await?);

        match SlicedPacket::from_ip(buf) {
            Err(e) => println!("parse packet: {:?}", e),
            Ok(packet) => match packet.net {
                Some(NetSlice::Ipv4(s)) => {
                    let ipheader = s.header();
                    if ipheader.protocol() == IpNumber::ICMP {
                        println!(
                            "got icmp {} -> {}",
                            ipheader.source_addr(),
                            ipheader.destination_addr()
                        );

                        let icmp_slice = Icmpv4Slice::from_slice(s.payload().payload)?;
                        match icmp_slice.icmp_type() {
                            Icmpv4Type::EchoRequest(r) => {
                                let builder = PacketBuilder::ipv4(
                                    ipheader.destination(),
                                    ipheader.source(),
                                    ipheader.ttl(),
                                )
                                .icmpv4_echo_reply(r.id, r.seq);

                                let payload = icmp_slice.payload();
                                let mut result =
                                    Vec::<u8>::with_capacity(builder.size(payload.len()));
                                builder.write(&mut result, payload)?;

                                device.send(&result).await?;
                            }
                            _ => {}
                        };
                    }
                }
                _ => {}
            },
        }
    }
}
