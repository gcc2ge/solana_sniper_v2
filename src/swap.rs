use crate::raydium_sdk;
use crate::redis::LiquidityPoolKeysString;
use crate::utils;
use crate::redis;
use crate::rugcheck;
use solana_client::rpc_client::RpcClient;
use std::str::FromStr;
use std::convert::From;
use raydium_sdk::MarketStateLayoutV3;
use raydium_sdk::get_associated_authority;
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use solana_transaction_status::EncodedTransaction;
use solana_transaction_status::UiMessage;
use solana_transaction_status::UiInstruction;
use solana_transaction_status::UiTransactionTokenBalance;
use solana_transaction_status::UiParsedInstruction;
use solana_transaction_status::option_serializer::OptionSerializer;
use solana_transaction_status::UiInnerInstructions;
use solana_transaction_status::parse_instruction::ParsedInstruction;
use rugcheck::{ check_burnt_lp, pre_rug_check };
use utils::fix_relaxed_json_in_lp_log_entry;
use utils::PoolInfo;
use utils::find_log_entry;
use std::error::Error;
use thiserror::Error;
use serde_json::{ Value, Result as JsonResult };
use redis::buy;
use std::sync::Arc;
use borsh::BorshDeserialize;
use redis::BuyTransaction;

// Define a custom error type for your application
#[derive(Debug, Error)]
pub enum PoolError {
    #[error("Base mint is SOL")]
    BaseMintIsSOL,
    #[error("Rug detected")]
    RugDetected,
    #[error("LP is not burnt")]
    LPNotBurnt,
    #[error("No pool info found")]
    NoPoolInfoFound,
    #[error("{0}")] Other(Box<dyn Error>), // Generic variant for other errors
}

pub async fn check_for_new_pool(
    tx: EncodedConfirmedTransactionWithStatusMeta,
    rpc_client: &Arc<RpcClient>,
    sol_amount: f64
) -> Result<String, PoolError> {
    let inner_instructions: Vec<UiInnerInstructions> = tx.transaction.meta
        .as_ref()
        .and_then(|data| {
            match &data.inner_instructions {
                OptionSerializer::Some(inner) => Some(inner.clone()),
                _ => None,
            }
        })
        .unwrap();

    let log_messages: Vec<String> = tx.transaction.meta
        .as_ref()
        .and_then(|data| {
            match &data.log_messages {
                OptionSerializer::Some(inner) => Some(inner.clone()),
                _ => None,
            }
        })
        .unwrap();
    let pre_token_balances: Vec<UiTransactionTokenBalance> = tx.transaction.meta
        .as_ref()
        .and_then(|data| {
            match &data.pre_token_balances {
                OptionSerializer::Some(inner) => Some(inner.clone()),
                _ => None,
            }
        })
        .unwrap();

    let sol_pubkey: Pubkey = Pubkey::from_str(
        "So11111111111111111111111111111111111111112"
    ).unwrap();
    let raydium_pubkey = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();

    let info: Option<PoolInfo> = parse_pool_info_from_lp_transaction(
        tx,
        &inner_instructions,
        &raydium_pubkey,
        &sol_pubkey,
        &log_messages,
        &pre_token_balances
    );

    if let Some(pool_info) = info {
        if pool_info.base_mint.to_string() == "So11111111111111111111111111111111111111112" {
            return Err(PoolError::BaseMintIsSOL);
        }
        println!("Kigger på {}", pool_info.base_mint);

        // First, check if it's a rug
        match pre_rug_check(&rpc_client, &pool_info.base_mint).await {
            Ok(is_rug) => {
                if is_rug {
                    dbg!("Rug detected");
                    return Err(PoolError::RugDetected);
                } else {
                    let is_lp_burnt = match check_burnt_lp(&rpc_client, &pool_info).await {
                        Ok(burnt) => burnt,
                        Err(err) => {
                            return Err(PoolError::Other(err.into()));
                        }
                    };
                    dbg!("Not rug checking LP");
                    if is_lp_burnt {
                        // Finally, fetch market info and perform the swap
                        let market_info = match
                            fetch_market_info(Arc::clone(&rpc_client), pool_info.market_id).await
                        {
                            Ok(market_info) => market_info,
                            Err(err) => {
                                return Err(PoolError::Other(err.into()));
                            }
                        };

                        let keyz: LiquidityPoolKeysString = create_pool_key(
                            &pool_info,
                            &market_info
                        );

                        dbg!("Købeer");

                        let buy_transaction = BuyTransaction {
                            in_token: pool_info.base_mint.to_string(),
                            out_token: pool_info.quote_mint.to_string(),
                            amount_in: sol_amount,
                            key_z: keyz,
                            type_: "buy".to_string(),
                            lp_decimals: pool_info.lp_decimals,
                        };

                        buy(buy_transaction);

                        return Ok("Success".to_string());
                    } else {
                        return Err(PoolError::LPNotBurnt);
                    }
                }
            }
            Err(err) => {
                return Err(PoolError::Other(err.into()));
            }
        }
    } else {
        return Err(PoolError::NoPoolInfoFound);
    }
}

fn create_pool_key(info: &PoolInfo, market_info: &MarketStateLayoutV3) -> LiquidityPoolKeysString {
    let market_auth = get_associated_authority(&info.market_program_id, &info.market_id);

    let pool_key: LiquidityPoolKeysString = LiquidityPoolKeysString {
        id: info.id.to_string(),
        base_mint: info.base_mint.to_string(),
        quote_mint: info.quote_mint.to_string(),
        lp_mint: info.lp_mint.to_string(),
        base_decimals: info.base_decimals,
        quote_decimals: info.quote_decimals,
        lp_decimals: info.lp_decimals,
        version: info.version,
        program_id: info.program_id.to_string(),
        authority: info.authority.to_string(),
        open_orders: info.open_orders.to_string(),
        target_orders: info.target_orders.to_string(),
        base_vault: info.base_vault.to_string(),
        quote_vault: info.quote_vault.to_string(),
        withdraw_queue: info.withdraw_queue.to_string(),
        lp_vault: info.lp_vault.to_string(),
        market_version: info.market_version,
        market_program_id: info.market_program_id.to_string(),
        market_id: info.market_id.to_string(),
        market_authority: market_auth.expect("Market_Auth").to_string(),
        market_base_vault: market_info.base_vault.to_string(),
        market_quote_vault: market_info.quote_vault.to_string(),
        market_bids: market_info.bids.to_string(),
        market_asks: market_info.asks.to_string(),
        market_event_queue: market_info.event_queue.to_string(),
    };

    pool_key
}

async fn fetch_market_info(
    client: Arc<RpcClient>,
    market_id: Pubkey
) -> Result<MarketStateLayoutV3, Box<dyn Error>> {
    let market_account_info = match client.get_account(&market_id) {
        Ok(account) => account,
        Err(err) => {
            return Err(err.into());
        }
    };

    let data: Vec<u8> = market_account_info.data;
    if data.is_empty() {
        return Err("Failed to fetch market info: empty data".into());
    }

    let market_state = MarketStateLayoutV3::try_from_slice(&data).map_err(|e|
        format!("Failed to decode market state: {}", e)
    )?;

    Ok(market_state)
}

fn parse_pool_info_from_lp_transaction(
    tx: EncodedConfirmedTransactionWithStatusMeta,
    inner_instructions: &Vec<UiInnerInstructions>,
    raydium_program_id: &Pubkey,
    wrapped_sol: &Pubkey,
    log_msg: &Vec<String>,
    pre_token_balances: &Vec<UiTransactionTokenBalance>
) -> Option<PoolInfo> {
    match tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            let message = ui_tx.message;

            match message {
                UiMessage::Parsed(ins) => {
                    let instructions: Vec<UiInstruction> = ins.instructions;
                    let init_instructions = find_instruction_by_program_id(
                        &instructions,
                        &raydium_program_id
                    );

                    let init_instructions = match init_instructions {
                        Some(instructions) => instructions,
                        None => {
                            return None;
                        }
                    };

                    match init_instructions {
                        UiInstruction::Parsed(inside) =>
                            match inside {
                                UiParsedInstruction::PartiallyDecoded(parsed) => {
                                    let token_program_id: &'static str =
                                        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
                                    let sol_decimals: u8 = 9;
                                    let base_mint = &parsed.accounts[8];
                                    let base_vault = &parsed.accounts[10];
                                    let quote_mint = &parsed.accounts[9];
                                    let quote_vault = &parsed.accounts[11];
                                    let lp_mint = &parsed.accounts[7];
                                    let base_and_quote_swapped =
                                        base_mint.to_string() == wrapped_sol.to_string();
                                    let lp_init_mint_instruction =
                                        find_initialize_mint_in_inner_instructions_by_mint_address(
                                            &inner_instructions,
                                            lp_mint
                                        );
                                    let lp_mint_mint_instruction =
                                        find_mint_in_inner_instructions_by_mint_address(
                                            &inner_instructions,
                                            lp_mint
                                        );

                                    match lp_mint_mint_instruction {
                                        Some(_) => {
                                            let base_transfer_instruction: Option<&ParsedInstruction> =
                                                find_transfer_instruction_in_inner_instructions_by_destination(
                                                    &inner_instructions,
                                                    base_vault,
                                                    Some(token_program_id)
                                                );
                                            let quote_transfer_instruction =
                                                find_transfer_instruction_in_inner_instructions_by_destination(
                                                    &inner_instructions,
                                                    quote_vault,
                                                    Some(token_program_id)
                                                );
                                            let lp_initialization_log_entry_info: Value =
                                                extract_lp_initialization_log_entry_info_from_log_entry(
                                                    find_log_entry("init_pc_amount", log_msg)
                                                        .unwrap()
                                                        .to_string()
                                                ).expect("error_lp_initialization_log_entry_info");
                                            let lp_decimals: u8 = get_decimals(
                                                &lp_init_mint_instruction
                                            ).expect("wrong_lp_decimals");
                                            let lp_ac: String = get_info_ac(
                                                &lp_mint_mint_instruction
                                            ).expect("lp_ac error");
                                            let open_time: u64 = extract_open_time(
                                                &lp_initialization_log_entry_info
                                            ).expect("open_time err");
                                            let base_pre_balance = find_base_pre_balance(
                                                pre_token_balances,
                                                &base_mint.to_string()
                                            );
                                            let base_decimals: u8 = get_base_decimals(
                                                &base_pre_balance
                                            );
                                            let base_reserves: String = get_info_amount(
                                                &base_transfer_instruction
                                            ).expect("err_base_reserves");
                                            let quote_reserves: String = get_info_amount(
                                                &quote_transfer_instruction
                                            ).expect("err_quote_reserves");
                                            let lp_reserves: String = get_info_amount(
                                                &lp_mint_mint_instruction
                                            ).expect("reserves_err");
                                            let pool_info = PoolInfo::new(
                                                Pubkey::from_str(
                                                    &parsed.accounts[4].clone()
                                                ).unwrap(),
                                                Pubkey::from_str(&base_mint).unwrap(),
                                                Pubkey::from_str(&quote_mint).unwrap(),
                                                Pubkey::from_str(&lp_mint).unwrap(),
                                                if base_and_quote_swapped {
                                                    sol_decimals
                                                } else {
                                                    base_decimals
                                                },
                                                if base_and_quote_swapped {
                                                    base_decimals
                                                } else {
                                                    sol_decimals
                                                },
                                                lp_decimals,
                                                4, // version
                                                Pubkey::from_str(
                                                    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"
                                                ).unwrap(),
                                                Pubkey::from_str(&parsed.accounts[5]).unwrap(),
                                                Pubkey::from_str(&parsed.accounts[6]).unwrap(),
                                                Pubkey::from_str(&parsed.accounts[13]).unwrap(),
                                                Pubkey::from_str(&base_vault).unwrap(),
                                                Pubkey::from_str(&quote_vault).unwrap(),
                                                Pubkey::from_str(
                                                    "11111111111111111111111111111111"
                                                ).unwrap(), // withdraw_queue
                                                Pubkey::from_str(lp_ac.as_str()).unwrap(), // lp_vault
                                                3, // market_version
                                                Pubkey::from_str(&parsed.accounts[15]).unwrap(),
                                                Pubkey::from_str(&parsed.accounts[16]).unwrap(),
                                                parse_amount(base_reserves.as_str()).unwrap(),
                                                parse_amount(quote_reserves.as_str()).unwrap(),
                                                parse_amount(&lp_reserves).unwrap(),
                                                open_time
                                            );
                                            Some(pool_info)
                                        }
                                        None => { None }
                                    }
                                }
                                _ => { None }
                            }

                        _ => { None }
                    }
                }
                _ => { None }
            }
        }
        _ => { None }
    }
}

fn find_instruction_by_program_id<'a>(
    instructions: &'a Vec<UiInstruction>,
    program_id: &'a Pubkey
) -> Option<&'a UiInstruction> {
    instructions.iter().find(|instr| {
        match instr {
            UiInstruction::Parsed(parsed_instr) =>
                match parsed_instr {
                    UiParsedInstruction::PartiallyDecoded(decoded_instr) => {
                        let decoded_program_id = Pubkey::from_str(&decoded_instr.program_id)
                            .ok()
                            .unwrap();
                        &decoded_program_id == program_id
                    }
                    &UiParsedInstruction::Parsed(_) => { false }
                }
            &&UiInstruction::Compiled(_) => todo!(),
        }
    })
}

fn find_transfer_instruction_in_inner_instructions_by_destination<'a>(
    inner_instructions: &'a Vec<UiInnerInstructions>,
    destination_account: &'a str,
    program_id: Option<&str>
) -> Option<&'a ParsedInstruction> {
    for inner in inner_instructions {
        for instruction in &inner.instructions {
            if let UiInstruction::Parsed(UiParsedInstruction::Parsed(instruct)) = instruction {
                let program_ide = &instruct.program_id;
                let data = &instruct.parsed;

                if
                    let (Some(type_field), Some(destination_field)) = (
                        extract_type_field(data),
                        extract_destination_from_info(data),
                    )
                {
                    if
                        type_field == "transfer" &&
                        destination_field == destination_account &&
                        (program_id.is_none() || program_ide == program_id.unwrap())
                    {
                        return Some(instruct);
                    }
                }
            }
        }
    }
    None
}

fn find_initialize_mint_in_inner_instructions_by_mint_address<'a>(
    inner_instructions: &'a Vec<UiInnerInstructions>,
    mint_address: &'a str
) -> Option<&'a ParsedInstruction> {
    for inner in inner_instructions {
        for instruction in &inner.instructions {
            if let UiInstruction::Parsed(UiParsedInstruction::Parsed(instruct)) = instruction {
                let data = &instruct.parsed;

                if
                    let (Some(type_field), Some(mint_field)) = (
                        extract_type_field(data),
                        extract_mint_from_info(data),
                    )
                {
                    if type_field == "initializeMint" && mint_field == mint_address {
                        return Some(instruct);
                    }
                }
            }
        }
    }
    None
}

fn find_mint_in_inner_instructions_by_mint_address<'a>(
    inner_instructions: &'a Vec<UiInnerInstructions>,
    mint_address: &'a str
) -> Option<&'a ParsedInstruction> {
    for inner in inner_instructions {
        for instruction in &inner.instructions {
            if let UiInstruction::Parsed(UiParsedInstruction::Parsed(instruct)) = instruction {
                let data = &instruct.parsed;

                if
                    let (Some(type_field), Some(mint_field)) = (
                        extract_type_field(data),
                        extract_mint_from_info(data),
                    )
                {
                    if type_field == "mintTo" && mint_field == mint_address {
                        return Some(instruct);
                    }
                }
            }
        }
    }
    None
}

fn extract_lp_initialization_log_entry_info_from_log_entry(
    lp_log_entry: String
) -> JsonResult<Value> {
    let lp_initialization_log_entry_info_start = lp_log_entry.find('{').unwrap_or(0);
    let json_str = &lp_log_entry[lp_initialization_log_entry_info_start..];
    // Assuming `fix_relaxed_json_in_lp_log_entry` is another function you have.
    // Replace it with the correct logic to fix the JSON string.
    let fixed_json_str = fix_relaxed_json_in_lp_log_entry(json_str);
    serde_json::from_str(&fixed_json_str)
}

fn get_decimals(lp_instruction: &Option<&ParsedInstruction>) -> Option<u8> {
    let ptx: &ParsedInstruction = lp_instruction.unwrap();
    let data: &Value = &ptx.parsed;

    return extract_decimals(&data);
}
fn get_info_ac(lp_instruction: &Option<&ParsedInstruction>) -> Option<String> {
    let ptx: &ParsedInstruction = lp_instruction.unwrap();
    let data: &Value = &ptx.parsed;
    return extract_ac_from_info(&data);
}
fn get_info_amount(base_instruction: &Option<&ParsedInstruction>) -> Option<String> {
    let ptx: &ParsedInstruction = base_instruction.unwrap();
    let data: &Value = &ptx.parsed;
    return extract_amount_from_info(&data);
}

fn get_base_decimals(base_pre_balance: &Option<UiTransactionTokenBalance>) -> u8 {
    let ptx: &UiTransactionTokenBalance = base_pre_balance.as_ref().unwrap();
    let data: u8 = ptx.ui_token_amount.decimals;
    return data;
}

fn find_base_pre_balance(
    pre_token_balances: &Vec<UiTransactionTokenBalance>,
    base_mint: &str
) -> Option<UiTransactionTokenBalance> {
    pre_token_balances
        .iter()
        .find(|balance| balance.mint == base_mint)
        .cloned()
}

fn extract_type_field(data: &Value) -> Option<String> {
    data.get("type").and_then(Value::as_str).map(String::from)
}
fn extract_open_time(data: &serde_json::Value) -> Option<u64> {
    data.get("open_time").and_then(|v| v.as_u64())
}
fn extract_decimals(data: &serde_json::Value) -> Option<u8> {
    data.get("info")
        .and_then(|info| info.get("decimals"))
        .and_then(serde_json::Value::as_u64)
        .map(|decimals| decimals as u8)
}
fn extract_mint_from_info(data: &Value) -> Option<String> {
    data.get("info")
        .and_then(|info| info.get("mint"))
        .and_then(Value::as_str)
        .map(String::from)
}
fn extract_destination_from_info(data: &Value) -> Option<String> {
    data.get("info")
        .and_then(|info| info.get("destination"))
        .and_then(Value::as_str)
        .map(String::from)
}
fn extract_ac_from_info(data: &Value) -> Option<String> {
    let info = data.get("info")?.as_object()?;

    let account = info.get("account")?.as_str()?;
    return Some(account.to_string());
}
fn extract_amount_from_info(data: &Value) -> Option<String> {
    data.get("info")
        .and_then(|info| info.get("amount"))
        .and_then(Value::as_str)
        .map(String::from)
}
fn parse_amount(amount_str: &str) -> Option<u64> {
    u64::from_str(amount_str).ok()
}
