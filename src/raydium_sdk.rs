use borsh::{ BorshDeserialize, BorshSerialize };
use solana_sdk::pubkey::Pubkey;
use serde::{ Serialize, Deserialize };

#[derive(Debug, Serialize, Deserialize)]
pub struct LiquidityPoolKeysString {
    id: String,
    base_mint: String,
    quote_mint: String,
    lp_mint: String,
    base_decimals: u8,
    quote_decimals: u8,
    lp_decimals: u8,
    version: u8,
    program_id: String,
    authority: String,
    open_orders: String,
    target_orders: String,
    base_vault: String,
    quote_vault: String,
    withdraw_queue: String,
    lp_vault: String,
    market_version: u8,
    market_program_id: String,
    market_id: String,
    market_authority: String,
    market_base_vault: String,
    market_quote_vault: String,
    market_bids: String,
    market_asks: String,
    market_event_queue: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct LiquidityPoolKeys {
    pub id: Pubkey,
    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,
    pub lp_mint: Pubkey,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub lp_decimals: u8,
    pub version: u8,
    pub program_id: Pubkey,
    pub authority: Pubkey,
    pub open_orders: Pubkey,
    pub target_orders: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub withdraw_queue: Pubkey,
    pub lp_vault: Pubkey,
    pub market_version: u8,
    pub market_program_id: Pubkey,
    pub market_id: Pubkey,
    pub market_authority: Pubkey,
    pub market_base_vault: Pubkey,
    pub market_quote_vault: Pubkey,
    pub market_bids: Pubkey,
    pub market_asks: Pubkey,
    pub market_event_queue: Pubkey,
}

impl From<LiquidityPoolKeys> for LiquidityPoolKeysString {
    fn from(pool_keys: LiquidityPoolKeys) -> Self {
        LiquidityPoolKeysString {
            id: pool_keys.id.to_string(),
            base_mint: pool_keys.base_mint.to_string(),
            quote_mint: pool_keys.quote_mint.to_string(),
            lp_mint: pool_keys.lp_mint.to_string(),
            base_decimals: pool_keys.base_decimals,
            quote_decimals: pool_keys.quote_decimals,
            lp_decimals: pool_keys.lp_decimals,
            version: pool_keys.version,
            program_id: pool_keys.program_id.to_string(),
            authority: pool_keys.authority.to_string(),
            open_orders: pool_keys.open_orders.to_string(),
            target_orders: pool_keys.target_orders.to_string(),
            base_vault: pool_keys.base_vault.to_string(),
            quote_vault: pool_keys.quote_vault.to_string(),
            withdraw_queue: pool_keys.withdraw_queue.to_string(),
            lp_vault: pool_keys.lp_vault.to_string(),
            market_version: pool_keys.market_version,
            market_program_id: pool_keys.market_program_id.to_string(),
            market_id: pool_keys.market_id.to_string(),
            market_authority: pool_keys.market_authority.to_string(),
            market_base_vault: pool_keys.market_base_vault.to_string(),
            market_quote_vault: pool_keys.market_quote_vault.to_string(),
            market_bids: pool_keys.market_bids.to_string(),
            market_asks: pool_keys.market_asks.to_string(),
            market_event_queue: pool_keys.market_event_queue.to_string(),
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct SwapInstructionData {
    instruction: u8,
    amount_in: u64,
    min_amount_out: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct MarketStateLayoutV3 {
    pub _padding: [u8; 13],

    pub own_address: Pubkey,
    pub vault_signer_nonce: u64,

    pub base_mint: Pubkey,
    pub quote_mint: Pubkey,

    pub base_vault: Pubkey,
    pub base_deposits_total: u64,
    pub base_fees_accrued: u64,

    pub quote_vault: Pubkey,
    pub quote_deposits_total: u64,
    pub quote_fees_accrued: u64,

    pub quote_dust_threshold: u64,

    pub request_queue: Pubkey,
    pub event_queue: Pubkey,

    pub bids: Pubkey,
    pub asks: Pubkey,

    pub base_lot_size: u64,
    pub quote_lot_size: u64,

    pub fee_rate_bps: u64,

    pub referrer_rebates_accrued: u64,

    _padding_end: [u8; 7],
}

pub fn get_associated_authority(
    program_id: &Pubkey,
    market_id: &Pubkey
) -> std::result::Result<Pubkey, String> {
    let market_id_bytes = market_id.to_bytes();
    let seeds = &[&market_id_bytes[..]];

    for nonce in 0..100u8 {
        let nonce_bytes = [nonce];
        let padding = [0u8; 7];

        let seeds_with_nonce = [
            seeds[0], // Market ID bytes
            &nonce_bytes, // Nonce bytes
            &padding, // Padding bytes
        ];

        match Pubkey::create_program_address(&seeds_with_nonce, program_id) {
            Ok(public_key) => {
                return Ok(public_key);
            }
            Err(_) => {
                continue;
            }
        }
    }

    Err("Unable to find a valid program address".into())
}
