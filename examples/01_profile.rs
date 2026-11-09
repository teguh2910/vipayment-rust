//! Example: Get Profile Information
//!
//! Run with:
//! ```bash
//! cargo run --example 01_profile
//! ```

use std::env;
use vipayment::VipaymentClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_id = env::var("VIPAYMENT_API_ID").unwrap_or_else(|_| "your_api_id".to_string());
    let api_key = env::var("VIPAYMENT_API_KEY").unwrap_or_else(|_| "your_api_key".to_string());

    let client = VipaymentClient::new(api_id, api_key);

    println!("=== 1. Get Profile Information ===");
    println!("API ID: {}", client.api_id());
    println!("Signature: {}", client.signature());

    let response = client.profile().await?;

    if response.is_success() {
        println!("Success!");
        if let Some(profile) = response.data {
            println!("Username: {:?}", profile.username);
            println!("Name: {:?}", profile.name);
            println!("Balance: {:?}", profile.balance);
            println!("Level: {:?}", profile.level);
            println!("Status: {:?}", profile.status);
            if !profile.extra.is_empty() {
                println!("Extra fields: {:?}", profile.extra);
            }
        }
    } else {
        println!("Failed: {}", response.message);
    }

    Ok(())
}
