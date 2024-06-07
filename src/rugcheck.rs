use crate::utils;
use solana_sdk::program_pack::Pack;
use spl_token::state::Mint;
use reqwest::Client;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use std::str::FromStr;
use tokio::time::sleep;
use solana_client::rpc_client::RpcClient;
use serde::Deserialize;
use utils::PoolInfo;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct TokenReport {
    mint: String,
    token: SplToken,
    lp: Option<LpInfo>,
    topHolders: Option<Vec<Holder>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct SplToken {
    freezeAuthority: Option<String>,
    mintAuthority: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
struct LpInfo {
    lpLockedPct: u8,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Holder {
    pct: f32,
    owner: String,
}

pub async fn check_burnt_lp(
    client: &RpcClient,
    pool_info: &PoolInfo
) -> Result<bool, Box<dyn std::error::Error>> {
    let lp_mint = pool_info.lp_mint;
    let lp_reserve = pool_info.lp_reserve;
    let timeout = Duration::from_secs(220); // 3 minutes
    let retry_interval = Duration::from_secs(15); // Retry every 5 seconds
    let start_time = tokio::time::Instant::now();

    loop {
        // Check elapsed time
        if tokio::time::Instant::now().duration_since(start_time) > timeout {
            return Ok(false);
        }

        // Get the mint info
        let acc_info = match client.get_account(&lp_mint) {
            Ok(info) => info,
            Err(_) => {
                sleep(retry_interval).await;
                continue;
            }
        };

        let mint_info = match Mint::unpack(acc_info.data.as_slice()) {
            Ok(info) => info,
            Err(_) => {
                sleep(retry_interval).await;
                continue;
            }
        };

        // Calculate reserve and actual supply
        let lp_reserve_amount = (lp_reserve as f64) / (10_f64).powi(mint_info.decimals as i32);
        let actual_supply = (mint_info.supply as f64) / (10_f64).powi(mint_info.decimals as i32);

        // Calculate burn amount and percentage
        let burn_amt = lp_reserve_amount - actual_supply;
        let burn_pct = (burn_amt / lp_reserve_amount) * 100.0;

        if burn_pct > 80.0 {
            return Ok(true);
        }

        // Wait before retrying
        sleep(retry_interval).await;
    }
}

pub async fn fetch_token_report(token: &str) -> Result<TokenReport, reqwest::Error> {
    let url = format!("https://api.rugcheck.xyz/v1/tokens/{}/report", token);
    let client = Client::new();
    let response = client.get(&url).header("accept", "application/json").send().await?;
    let report = response.json().await?;
    Ok(report)
}

pub async fn check_rug_sol(
    client: &RpcClient,
    token: &str
) -> Result<bool, Box<dyn std::error::Error>> {
    let token_pubkey = Pubkey::from_str(token)?;
    let mint_account = client.get_account(&token_pubkey)?;
    let mint_data = mint_account.data.as_slice();

    let mint_token = Mint::unpack(mint_data)?;
    let mint_authority = mint_token.mint_authority;
    let freeze_authority = mint_token.freeze_authority;

    let is_rug = mint_authority.is_some() || freeze_authority.is_some();
    Ok(is_rug)
}

pub async fn rug_check(
    client: &RpcClient,
    token: &str
) -> Result<bool, Box<dyn std::error::Error>> {
    if token == "So11111111111111111111111111111111111111112" {
        return Ok(false); // This token is not a rug
    }

    // First, try fetching the token report
    let report = match fetch_token_report(token).await {
        Ok(report) => report,
        Err(_) => {
            // If no report is found, fall back to checking via Solana
            return check_rug_sol(client, token).await;
        }
    };

    // If report is found, proceed with its analysis
    let freeze_authority = report.token.freezeAuthority;
    let mint_authority = report.token.mintAuthority;

    // Check if Raydium is the largest holder and exclude it from the large holder check
    let raydium_address = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
    let top_holders = report.topHolders.unwrap_or_default();
    let raydium_is_largest = top_holders
        .first()
        .map_or(false, |holder| holder.owner == raydium_address);

    if !raydium_is_largest {
        return Ok(true); // Consider it a rug if Raydium is not the largest holder
    }

    // Check if any of the top holders (excluding Raydium) have more than 20%
    let large_holder = top_holders
        .iter()
        .filter(|holder| holder.owner != raydium_address)
        .any(|holder| holder.pct > 20.0);

    // If no large holder is found and both authorities are None, return false (not a rug)
    if !large_holder && freeze_authority.is_none() && mint_authority.is_none() {
        Ok(false)
    } else {
        Ok(true) // Otherwise, consider it as a rug
    }
}
