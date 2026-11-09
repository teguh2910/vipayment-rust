//! Example: Game & Streaming Services
//!
//! Run with:
//! ```bash
//! cargo run --example 03_game
//! ```

use std::env;
use vipayment::VipaymentClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_id = env::var("VIPAYMENT_API_ID").unwrap_or_else(|_| "your_api_id".to_string());
    let api_key = env::var("VIPAYMENT_API_KEY").unwrap_or_else(|_| "your_api_key".to_string());

    let client = VipaymentClient::new(api_id, api_key);

    // -------------------------------------------------------------------------
    // 1. Check Available Game Services
    // -------------------------------------------------------------------------
    println!("\n=== 1. Check Available Game Services ===");
    let services = client
        .service_game(Some("Mobile Legends"), Some("available"))
        .await?;

    if services.is_success() {
        if let Some(data) = services.data {
            println!("Found {} services.", data.len());
            for service in data.iter().take(3) {
                println!(
                    "- [{}] {:?} (Price: {:?})",
                    service.code.as_deref().unwrap_or("-"),
                    service.name.as_deref().unwrap_or("-"),
                    service.price
                );
            }
        }
    } else {
        println!("Failed to get services: {}", services.message);
    }

    // -------------------------------------------------------------------------
    // 2. Order Game Top-up (Example)
    // -------------------------------------------------------------------------
    println!("\n=== 2. Order Game Top-up (Example) ===");
    let service_code = "ML5";
    let user_id = "12345678";
    let zone_id = Some("1234");

    println!(
        "(Game order skipped: uncomment below to order {} for user {} zone {:?})",
        service_code, user_id, zone_id
    );
    /*
    let order = client.order_game(service_code, user_id, zone_id).await?;
    if order.is_success() {
        if let Some(order_data) = order.data {
            println!("Order Success! TrxID: {:?}", order_data.trxid);
        }
    } else {
        println!("Order Failed: {}", order.message);
    }
    */

    // -------------------------------------------------------------------------
    // 3. Check Game Order Status
    // -------------------------------------------------------------------------
    println!("\n=== 3. Check Game Order Status ===");
    let trx_id = "TRX987654321";

    println!(
        "(Game status check skipped: uncomment below to check {})",
        trx_id
    );
    /*
    let status = client.status_order_game(trx_id, None).await?;
    if status.is_success() {
        if let Some(items) = status.data {
            for item in items {
                println!("Status: {:?}", item.status);
                println!("SN: {:?}", item.sn);
            }
        }
    } else {
        println!("Check Game Status Failed: {}", status.message);
    }
    */

    Ok(())
}
