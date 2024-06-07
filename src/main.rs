mod buy;
mod swap;
mod raydium_sdk;
mod utils;
mod redis;
mod mongo;
mod rugcheck;
use dotenv::dotenv;
use buy::listen_for_buys;
use solana_client::{ nonblocking::pubsub_client::PubsubClient, rpc_client::RpcClient };
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let wss_endpoint = std::env
        ::var("WSS_URL")
        .expect("You must set the WSS environment variable!");
    let rpc_endpoint = std::env
        ::var("RPC_URL")
        .expect("You must set the RPC_URL environment variable!");

    let program_address = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"; // RAYDIUM_PUBLIC_KEY
    let rpc_client: Arc<RpcClient> = Arc::new(RpcClient::new(rpc_endpoint.to_string()));
    let pubsub_client: PubsubClient = PubsubClient::new(&wss_endpoint).await?;
    let sol_amount = 0.005;
    listen_for_buys(rpc_client, pubsub_client, program_address, sol_amount).await?;
    Ok(())
}
