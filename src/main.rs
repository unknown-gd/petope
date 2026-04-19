use anyhow::Result;
use clap::{Parser, Subcommand};
use iroh::{Endpoint, endpoint::presets, protocol::Router};
use iroh_ping::Ping;
use iroh_tickets::{Ticket, endpoint::EndpointTicket};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Receive,
    Send { ticket: String },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Receive => run_receiver().await?,
        Commands::Send { ticket } => {
            let ticket = EndpointTicket::deserialize(&ticket)?;
            run_sender(ticket).await?;
        }
    }

    Ok(())
}

async fn run_receiver() -> Result<()> {
    let endpoint = Endpoint::bind(presets::N0).await?;
    endpoint.online().await;
    let ping = Ping::new();
    let ticket = EndpointTicket::new(endpoint.addr());
    println!("{ticket}");

    let _router = Router::builder(endpoint)
        .accept(iroh_ping::ALPN, ping)
        .spawn();

    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn run_sender(ticket: EndpointTicket) -> Result<()> {
    let send_ep = Endpoint::bind(presets::N0).await?;
    let send_pinger = Ping::new();
    let rtt = send_pinger
        .ping(&send_ep, ticket.endpoint_addr().clone())
        .await?;
    println!("ping took: {:?} to complete", rtt);
    Ok(())
}
