use crate::utils;
use solana_sdk::program_pack::Pack;
use spl_token::state::Mint;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;
use std::str::FromStr;
use solana_client::rpc_client::RpcClient;
use serde::Deserialize;
use utils::PoolInfo;
use std::error::Error;

#[derive(Debug, Deserialize)]
pub struct TokenReport {
    pub token: TokenInfo,
    pub top_holders: Option<Vec<TopHolder>>,
}

#[derive(Debug, Deserialize)]
pub struct TokenInfo {
    pub freeze_authority: Option<Pubkey>,
    pub mint_authority: Option<Pubkey>,
}

#[derive(Debug, Deserialize)]
pub struct TopHolder {
    pub owner: Pubkey,
    pub amount: u64,
    pub pct: f64,
}

pub async fn get_current_sol_price() -> Result<f64, Box<dyn Error>> {
    let url =
        "https://public-api.birdeye.so/defi/price?address=So11111111111111111111111111111111111111112";

    let birdeye_api_key = std::env
        ::var("BIRDEYE_API")
        .expect("You must set the RPC_URL environment variable!");
    // Make GET request to the API endpoint with API key in header
    let response = reqwest::Client
        ::new()
        .get(url)
        .header("X-API-KEY", birdeye_api_key)
        .send().await?;

    // Check if response is successful
    if response.status().is_success() {
        // Parse the JSON response to extract the SOL price
        let sol_price_json: serde_json::Value = response.json().await?;
        let sol_price_usd: f64 = sol_price_json["data"]["value"].as_f64().unwrap_or(0.0);
        Ok(sol_price_usd)
    } else {
        Err("Failed to fetch SOL price".into()) // Convert string to boxed Error trait object
    }
}

async fn calculate_liquidity_usd(
    client: &RpcClient,
    base_token_account: Pubkey,
    quote_token_account: Pubkey
) -> Result<f64, Box<dyn Error>> {
    let sol_price = get_current_sol_price().await?; // Get current SOL price

    // Get base token balance
    let base_token_info = match client.get_token_account_balance(&base_token_account) {
        Ok(info) => info,
        Err(err) => {
            return Err(Box::new(err));
        }
    };
    let base_balance = base_token_info.amount.parse::<f64>()?;

    // Get quote token balance
    let quote_token_info = match client.get_token_account_balance(&quote_token_account) {
        Ok(info) => info,
        Err(err) => {
            return Err(Box::new(err));
        }
    };
    let quote_balance = quote_token_info.amount.parse::<f64>()?;

    // Calculate base price in SOL
    let base_price_in_sol = quote_balance / base_balance;

    // Calculate base price in USD
    let base_price_in_usd = base_price_in_sol * sol_price;

    // Calculate USD value of base balance
    let value_usd_base_balance = base_balance * base_price_in_usd;

    // Calculate USD value of quote balance
    let value_usd_quote_balance = quote_balance * sol_price;

    // Calculate liquidity USD value
    let liquidity_usd = value_usd_base_balance + value_usd_quote_balance;

    let formatted_liquidity = liquidity_usd / 1000000000.0;

    Ok(formatted_liquidity)
}

pub async fn check_burnt_lp(
    client: &RpcClient,
    pool_info: &PoolInfo
) -> Result<bool, Box<dyn std::error::Error>> {
    let lp_mint = pool_info.lp_mint;
    let lp_reserve = pool_info.lp_reserve;
    let timeout = Duration::from_secs(220); // 3 minutes
    let retry_interval = Duration::from_secs(15); // Retry every 15 seconds
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
                tokio::time::sleep(retry_interval).await;
                continue;
            }
        };

        let mint_info = match Mint::unpack(acc_info.data.as_slice()) {
            Ok(info) => info,
            Err(_) => {
                tokio::time::sleep(retry_interval).await;
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
            let liquidity_usd = calculate_liquidity_usd(
                client,
                pool_info.base_vault,
                pool_info.quote_vault
            ).await?;

            return Ok(liquidity_usd > 1000.0); // Return true if liquidity is greater than $1000
        }

        // Wait before retrying
        tokio::time::sleep(retry_interval).await;
    }
}

pub async fn get_top_holders(
    client: &RpcClient,
    token: &Pubkey
) -> Result<Vec<TopHolder>, Box<dyn std::error::Error>> {
    let token_accounts = match client.get_token_largest_accounts(token) {
        Ok(accounts) => accounts,
        Err(err) => {
            return Err(err.into());
        }
    };

    let token_supply = match client.get_token_supply(token) {
        Ok(supply) => supply.amount.parse::<f64>().unwrap(),
        Err(err) => {
            return Err(err.into());
        }
    };

    let top_holders: Result<Vec<TopHolder>, Box<dyn std::error::Error>> = token_accounts
        .into_iter()
        .map(|account| {
            let owner = Pubkey::from_str(&account.address).unwrap();
            let amount = match account.amount.amount.parse::<u64>() {
                Ok(amount) => amount,
                Err(err) => {
                    return Err(err.into());
                }
            };
            let pct = match ((amount as f64) / token_supply) * 100.0 {
                pct if pct.is_nan() => {
                    return Err("Percentage calculation resulted in NaN".into());
                }
                pct => pct,
            };
            Ok(TopHolder {
                owner,
                amount,
                pct,
            })
        })
        .collect();
    let top_holders = top_holders.map_err(|err| err)?;
    Ok(top_holders)
}

pub async fn check_rug_sol(
    client: &RpcClient,
    token: &Pubkey
) -> Result<bool, Box<dyn std::error::Error>> {
    let mint_account = client.get_account(&token)?;
    let mint_data = mint_account.data.as_slice();

    let mint_token = Mint::unpack(mint_data)?;
    let mint_authority = mint_token.mint_authority;
    let freeze_authority = mint_token.freeze_authority;

    let is_rug = mint_authority.is_some() || freeze_authority.is_some();
    Ok(is_rug)
}

pub async fn pre_rug_check(
    client: &RpcClient,
    token: &Pubkey
) -> Result<bool, Box<dyn std::error::Error>> {
    if token.to_string() == "So11111111111111111111111111111111111111112" {
        return Ok(false); // This token is not a rug
    }

    let is_freeze_and_mint_disabled = check_rug_sol(client, token).await?;

    return Ok(is_freeze_and_mint_disabled);
}

async fn post_rug_check(
    client: &RpcClient,
    token: &Pubkey
) -> Result<bool, Box<dyn std::error::Error>> {
    // Check if Raydium is the largest holder and exclude it from the large holder check
    let raydium_address = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

    let top_holders = get_top_holders(client, token).await?;
    // Check if Raydium is the largest holder and exclude it from the large holder check
    let raydium_is_largest = top_holders
        .first()
        .map_or(false, |holder| holder.owner.to_string() == raydium_address);

    if !raydium_is_largest {
        return Ok(true); // Consider it a rug if Raydium is not the largest holder
    }

    // Check if any of the top holders (excluding Raydium) have more than 20%
    let large_holder = top_holders
        .iter()
        .filter(|holder| holder.owner.to_string() != raydium_address)
        .any(|holder| holder.pct > 20.0);

    // If no large holder is found and both authorities are None, return false (not a rug)
    if !large_holder {
        Ok(false)
    } else {
        Ok(true) // Otherwise, consider it as a rug
    }
}
