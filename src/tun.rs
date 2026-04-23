use std::{
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use crate::state::State;
use anyhow::{Context, Result};
use bytes::BytesMut;
use futures::{SinkExt, StreamExt};
use net_route::{Handle, Route};
use tokio::sync::mpsc;
use tun_rs::{
    AsyncDevice, DeviceBuilder,
    async_framed::{BytesCodec, DeviceFramed},
};

pub async fn create_tun(
    state: &State,
    addr: (Ipv4Addr, Ipv6Addr),
    incoming: mpsc::Sender<BytesMut>,
    mut outcoming: mpsc::Receiver<BytesMut>,
) -> Result<()> {
    let dev = create_device(&state, addr).context("create tun device")?;
    let ifindex = get_ifindex(&dev.name()?).context("retrieve tun device index")?;

    setup_routes(&state, ifindex)
        .await
        .context("setup routes")?;

    let (mut reader, mut writer) = DeviceFramed::new(Arc::new(dev), BytesCodec::new()).split();

    // read data from the tun
    tokio::spawn(async move {
        while let Some(frame) = reader.next().await {
            match frame {
                Ok(bytes) => incoming.send(bytes).await.unwrap(),
                Err(e) => eprintln!("unable to read tun frame: {:?}", e),
            }
        }
    });

    // send data to the tun
    tokio::spawn(async move {
        while let Some(bytes) = outcoming.recv().await {
            if outcoming.capacity() < 4 {
                println!(
                    "tun out backpressure, channel capacity {}/{}",
                    outcoming.capacity(),
                    outcoming.max_capacity()
                );
            }
            writer.send(bytes).await.unwrap();
        }
    });

    Ok(())
}

fn create_device(state: &State, addr: (Ipv4Addr, Ipv6Addr)) -> std::io::Result<AsyncDevice> {
    DeviceBuilder::new()
        .name(get_device_name(state)?)
        .mtu(1000)
        .ipv4(addr.0, 32, None)
        .ipv6(addr.1, 128)
        .layer(tun_rs::Layer::L3)
        .build_async()
}

async fn setup_routes(_state: &State, ifindex: u32) -> std::io::Result<()> {
    let handle = Handle::new()?;
    let route = Route::new("fdee::2".parse().unwrap(), 128).with_ifindex(ifindex);
    handle.add(&route).await?;

    Ok(())
}

fn get_device_prefix() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "utun"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "petope"
    }
}

fn get_device_name(state: &State) -> std::io::Result<String> {
    use getifs::SmolStr;
    use std::io::{Error, ErrorKind};

    if let Some(name) = &state.config.interface_name {
        return Ok(name.clone());
    }

    let prefix = get_device_prefix();

    // get all interfaces that start with the prefix
    let interfaces = getifs::interfaces()?
        .into_iter()
        .filter(|i| i.name().starts_with(prefix))
        .map(|i| i.name().clone())
        .collect::<Vec<SmolStr>>();

    for i in 0..100 {
        let name = format!("{}{}", prefix, i);

        // check if none of the interfaces have the name
        if !interfaces.iter().any(|v| v.as_str() == name) {
            return Ok(name);
        }
    }

    Err(Error::new(
        ErrorKind::Other,
        format!(
            "unable to find an available tun device name with prefix {}, already {} interfaces exist",
            prefix,
            interfaces.len()
        ),
    ))
}

fn get_ifindex(device_name: &str) -> std::io::Result<u32> {
    use std::io::{Error, ErrorKind};

    getifs::interface_by_name(device_name)
        .and_then(|v| {
            v.ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("interface {} not found", device_name),
                )
            })
        })
        .map(|i| i.index())
}
