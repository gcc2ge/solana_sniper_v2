use crate::utils::find_log_entry;
use crate::swap::check_for_new_pool;
use crate::mongo::MongoHandler;
use std::sync::Arc;
use solana_client::rpc_client::RpcClient;
use solana_client::{
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{ RpcTransactionConfig, RpcTransactionLogsConfig, RpcTransactionLogsFilter },
    rpc_response::RpcLogsResponse,
};
use std::time::Duration;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use solana_sdk::signature::Signature;
use tokio::time::sleep;
use futures::stream::StreamExt;
use solana_transaction_status::UiTransactionEncoding;
use std::str::FromStr;

pub async fn listen_for_buys(
    rpc_client: Arc<RpcClient>,
    pub_subclient: PubsubClient,
    program_address: &str,
    sol_amount: f64
) -> Result<(), Box<dyn std::error::Error + 'static>> {
    const MAX_RETRIES: usize = 3;
    const INITIAL_RETRY_DELAY: u64 = 2;
    const TOKEN_THRESHOLD: usize = 3;
    const THRESHOLD_SLEEP_DURATION: u64 = 300; // 5 minutes

    let (mut stream, _) = pub_subclient.logs_subscribe(
        RpcTransactionLogsFilter::Mentions(vec![program_address.to_string()]),
        RpcTransactionLogsConfig {
            commitment: Some(CommitmentConfig::processed()),
        }
    ).await?;

    let mut seen_transactions: Vec<String> = vec![];

    loop {
        match stream.next().await {
            Some(response) => {
                let logs: RpcLogsResponse = response.value;
                let log_entries = &logs.logs;

                if let Some(_found_entry) = find_log_entry("init_pc_amount", log_entries) {
                    let tx_signature = logs.signature.clone();
                    if seen_transactions.contains(&tx_signature) {
                        continue;
                    }
                    seen_transactions.push(tx_signature.clone());

                    let mut retry_count = 0;
                    let mut retry_delay = INITIAL_RETRY_DELAY;

                    loop {
                        let mongo_handler = MongoHandler::new().await.expect(
                            "Failed to create MongoHandler"
                        );
                        let tokens = mongo_handler.fetch_all_tokens("solsniper", "tokens").await?;
                        if tokens.len() > TOKEN_THRESHOLD {
                            sleep(Duration::from_secs(THRESHOLD_SLEEP_DURATION)).await;
                            dbg!("Threshold reached, sleeping for 5 minutes");
                            break; // Exit the retry loop
                        }

                        match try_get_transaction(&rpc_client, &tx_signature).await {
                            Ok(tx) => {
                                let _signature = check_for_new_pool(
                                    tx,
                                    &rpc_client,
                                    sol_amount
                                ).await;
                                break; // Exit the retry loop if transaction successful
                            }
                            Err(err) => {
                                retry_count += 1;

                                if retry_count > MAX_RETRIES {
                                    eprintln!("Failed to get transaction: {}", err);
                                    break; // Exit the retry loop if max retries exceeded
                                }

                                // Retry with exponential backoff delay
                                sleep(Duration::from_secs(retry_delay)).await;
                                retry_delay *= 2;
                            }
                        }
                    }
                }
            }
            None => {
                println!("End of stream");
                break; // Exit the loop when the stream ends
            }
        }
    }

    Ok(())
}

async fn try_get_transaction(
    rpc_client: &Arc<RpcClient>,
    tx_signature: &str
) -> Result<EncodedConfirmedTransactionWithStatusMeta, Box<dyn std::error::Error>> {
    let signature = Signature::from_str(&tx_signature)?;
    let config = RpcTransactionConfig {
        encoding: Some(UiTransactionEncoding::JsonParsed),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };
    let tx: EncodedConfirmedTransactionWithStatusMeta = rpc_client.get_transaction_with_config(
        &signature,
        config
    )?;
    Ok(tx)
}
