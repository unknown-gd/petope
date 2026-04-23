use crate::{config::Config, router::Router};
use anyhow::{Context, Result};
use clap::Parser;
use iroh::{Endpoint, endpoint::presets};

mod config;
mod packet;
mod router;
mod state;
mod tun;
mod utils;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to a config file
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    config: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let (secret_key, config) = Config::load(&cli.config).context("load config")?;

    let state = state::State::new(config);

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .bind()
        .await
        .context("bind an endpoint")?;

    let router = Router::run(&state, endpoint.clone())
        .await
        .context("run router")?;

    println!("running as {}", endpoint.id().to_z32());
    println!("ipv4: {} ipv6: {}", router.addr_v4, router.addr_v6);

    tokio::signal::ctrl_c().await?;
    println!("bye-bye");
    Ok(())
}
