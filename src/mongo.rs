use mongodb::{ Client, options::ClientOptions, bson::doc, bson::Document, Collection };
use mongodb::error::Error as MongoError;
use serde::Serialize;
use serde::Deserialize;
use futures::stream::TryStreamExt;
use mongodb::bson::DateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub balance: f64,
    pub mint: String,
    pub description: String,
    pub image: String,
    pub twitter: String,
    pub created_on: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyTransaction {
    pub transaction_signature: String,
    pub token_info: TokenInfo,
    pub amount: f64,
    pub sol_amount: f64,
    pub sol_price: f64,
    pub entry_price: f64,
    pub token_metadata: TokenMetadata,
    pub created_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SellTransaction {
    pub transaction_signature: String,
    pub token_info: TokenInfo,
    pub amount: f64,
    pub sol_amount: f64,
    pub sol_price: f64,
    pub sell_price: f64,
    pub profit: f64,
    pub profit_percentage: f64,
    pub created_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenInfo {
    pub base_mint: String,
    pub quote_mint: String,
    pub base_vault: String,
    pub quote_vault: String,
}

pub struct MongoHandler {
    client: Client,
}

impl MongoHandler {
    pub async fn new() -> Result<Self, MongoError> {
        // Load the MongoDB connection string from an environment variable
        let client_uri = std::env
            ::var("MONGODB_URI")
            .expect("You must set the MONGODB_URI environment variable!");

        // Parse the client options
        let options = ClientOptions::parse(&client_uri).await?;
        let client = Client::with_options(options)?;

        Ok(Self { client })
    }

    pub async fn fetch_all_tokens(
        &self,
        db_name: &str,
        collection_name: &str
    ) -> Result<Vec<TokenMetadata>, MongoError> {
        let my_coll: Collection<Document> = self.client
            .database(db_name)
            .collection(collection_name);

        let mut cursor = my_coll.find(doc! {}, None).await?;
        let mut tokens: Vec<TokenMetadata> = Vec::new(); // Initialize an empty vector to store tokens

        while let Some(doc) = cursor.try_next().await? {
            // Extract the `token_metadata` field from the document
            if let Some(token_metadata_bson) = doc.get("token_metadata") {
                // Check if the extracted value is a document
                let sold = doc.get("sold").unwrap().as_bool().unwrap();

                if sold == true {
                    continue;
                } else {
                    if let Some(token_metadata_doc) = token_metadata_bson.as_document() {
                        // Deserialize the document into TokenMetadata
                        if
                            let Ok(metadata_obj) = bson::from_document::<TokenMetadata>(
                                token_metadata_doc.clone()
                            )
                        {
                            tokens.push(metadata_obj);
                        } else {
                            return Err(
                                MongoError::custom(
                                    "Failed to deserialize token metadata".to_string()
                                )
                            );
                        }
                    } else {
                        return Err(
                            MongoError::custom("Token metadata field is not a document".to_string())
                        );
                    }
                }
            } else {
                return Err(MongoError::custom("Token metadata field not found".to_string()));
            }
        }

        Ok(tokens) // Return the vector of tokens
    }
}
