use crate::utils;
use anyhow::Result;
use bytes::BytesMut;
use clap::Args;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::{
    net::{SocketAddr, SocketAddrV4},
    process::exit,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::sleep,
};

const MDNS_SERVICE_TYPE: &str = "_petope._tcp.local.";

#[derive(Args, Debug)]
pub struct NodeArgs {
    id: String,
    target: String,

    // Discovery server address
    #[arg(long, short, default_value_t = String::from("127.0.0.1:4444"))]
    discovery: String,
}

pub async fn main(args: NodeArgs) -> Result<()> {
    let local_addr = {
        let listener = utils::reusable_socket(Some("0.0.0.0:0".parse()?))?.listen(8)?;
        let local_addr = listener.local_addr()?;
        let my_id = args.id.clone();

        tokio::spawn(async move {
            loop {
                let (mut stream, addr) = listener.accept().await.unwrap();
                println!("connection from {}", addr);

                stream
                    .write_all(format!("you are connected to {}", &my_id).as_bytes())
                    .await
                    .unwrap();

                let mut buf = BytesMut::new();
                stream.read_buf(&mut buf).await.unwrap();
                println!("message: {}", std::str::from_utf8(buf.as_ref()).unwrap());

                exit(0);
            }
        });

        local_addr
    };

    let sd = ServiceDaemon::new()?;

    {
        let hostname = utils::get_hostname()?;
        let service = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &args.id,
            &hostname,
            (),
            local_addr.port(),
            None,
        )?
        .enable_addr_auto();
        sd.register(service)?;
    }

    // let mut discovery = {
    //     let host = lookup_host(args.discovery)
    //         .await?
    //         .filter(|v| v.is_ipv4()) // no ipv6 support yet :p
    //         .next()
    //         .context("unable to lookup discovery address")?;

    //     let stream = utils::reusable_socket(Some(local_addr))?
    //         .connect(host)
    //         .await
    //         .with_context(|| format!("connect to discovery server via {}", host))?;

    //     DiscoveryClient::new(stream)
    // };

    // discovery.register(args.id.clone()).await?;
    // match discovery.get(args.id.clone()).await? {
    //     Some(public_addr) => {
    //         println!(
    //             "registered as \"{}\" with address {}",
    //             &args.id, public_addr
    //         )
    //     }
    //     None => bail!("discovery server failed to register us"),
    // };

    loop {
        if let Some(node_addr) = discover_node(&sd, &args.target).await.ok() {
            println!("trying to connect to {}", node_addr);
            match utils::reusable_socket(Some(local_addr))?
                .connect(node_addr)
                .await
            {
                Ok(mut stream) => {
                    println!("connected!");
                    stream
                        .write_all(format!("yo, i am {}", &args.id).as_bytes())
                        .await?;

                    let mut buf = BytesMut::new();
                    stream.read_buf(&mut buf).await?;
                    println!("message: {}", std::str::from_utf8(buf.as_ref()).unwrap());

                    sleep(Duration::from_secs(1)).await;
                    exit(0);
                }
                Err(e) => {
                    println!("unable due {}", e);
                }
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}

async fn discover_node(sd: &ServiceDaemon, node_id: &str) -> Result<SocketAddr> {
    let receiver = sd.browse(MDNS_SERVICE_TYPE)?;
    let node_fullname = format!("{}.{}", node_id, MDNS_SERVICE_TYPE);

    loop {
        let event = receiver.recv_async().await?;
        match event {
            ServiceEvent::ServiceResolved(svc) => {
                if svc.get_fullname() == node_fullname.as_str() {
                    let addresses = svc.get_addresses_v4();
                    println!("discovered {} with addresses {:?}", node_id, addresses);
                    sd.stop_browse(MDNS_SERVICE_TYPE)?;

                    return Ok(SocketAddr::V4(SocketAddrV4::new(
                        addresses.iter().next().unwrap().to_owned(),
                        svc.get_port(),
                    )));
                }
            }
            _ => {}
        };
    }
}
