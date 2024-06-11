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
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

async fn check_wsol_balance(
    rpc_client: &RpcClient,
    required_wsol: f64
) -> Result<bool, Box<dyn std::error::Error>> {
    let account_info = rpc_client.get_account(
        &Pubkey::from_str("3rzKBn91t3ttL23by55oo9h5Ag89nCdvFbHwvs58Uj52").unwrap()
    );

    match account_info {
        Ok(account) => {
            let lamports = account.lamports;
            let wsol_balance = (lamports as f64) / ((10u64).pow(9) as f64); // Convert lamports to WSOL
            Ok(wsol_balance >= required_wsol)
        }
        Err(_) => Ok(false),
    }
}

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
    let wsol_amount = 0.024;

    loop {
        // Check WSOL balance before listening for buys
        let enough_wsol = check_wsol_balance(&rpc_client, wsol_amount).await?;
        if enough_wsol {
            listen_for_buys(rpc_client.clone(), pubsub_client, program_address, wsol_amount).await?;

            break;
        } else {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await; // Sleep for 1 hour
        }
    }

    Ok(())
}
