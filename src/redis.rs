use redis::{ Client, Commands };
use serde_json;
use serde::{ Serialize, Deserialize };

#[derive(Debug, Serialize, Deserialize)]
pub struct LiquidityPoolKeysString {
    pub id: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub lp_mint: String,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub lp_decimals: u8,
    pub version: u8,
    pub program_id: String,
    pub authority: String,
    pub open_orders: String,
    pub target_orders: String,
    pub base_vault: String,
    pub quote_vault: String,
    pub withdraw_queue: String,
    pub lp_vault: String,
    pub market_version: u8,
    pub market_program_id: String,
    pub market_id: String,
    pub market_authority: String,
    pub market_base_vault: String,
    pub market_quote_vault: String,
    pub market_bids: String,
    pub market_asks: String,
    pub market_event_queue: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyTransaction {
    pub type_: String,
    pub in_token: String,
    pub out_token: String,
    pub amount_in: f64,
    pub key_z: LiquidityPoolKeysString,
    pub lp_decimals: u8,
}

// Adjust the buy function to accept BuyTransaction and LiquidityPoolKeysString
pub fn buy(transaction: BuyTransaction) {
    // Serialize the BuyTransaction object into JSON
    let transaction_json = serde_json::to_string(&transaction).unwrap();
    let redis_url = std::env
        ::var("REDIS_URL")
        .expect("You must set the REDIS_URL environment variable!");

    // Connect to Redis
    let client = Client::open(redis_url).unwrap();
    let mut connection = client.get_connection().unwrap();

    // Publish the JSON payload to the "trading" channel
    let _: usize = connection.publish("trading", transaction_json).unwrap();
}
