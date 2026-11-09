//! Example: Prepaid Services (Pulsa, Data, PLN, etc.)
//!
//! Run with:
//! ```bash
//! cargo run --example 02_prepaid
//! ```

use std::env;
use vipayment::VipaymentClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_id = env::var("VIPAYMENT_API_ID").unwrap_or_else(|_| "your_api_id".to_string());
    let api_key = env::var("VIPAYMENT_API_KEY").unwrap_or_else(|_| "your_api_key".to_string());

    let client = VipaymentClient::new(api_id, api_key);

    // -------------------------------------------------------------------------
    // 1. Check Available Prepaid Services
    // -------------------------------------------------------------------------
    println!("\n=== 1. Check Available Services (Pulsa Reguler) ===");
    let services = client
        .service_prepaid(Some("type"), Some("pulsa-reguler"))
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
    // 2. Order Prepaid Service (Example)
    // -------------------------------------------------------------------------
    println!("\n=== 2. Order Prepaid Service (Example) ===");
    let service_code = "PLN5";
    let destination_number = "1234567890";

    println!(
        "(Order skipped: set valid credentials and uncomment below to order {} for {})",
        service_code, destination_number
    );
    /*
    let order = client.order_prepaid(service_code, destination_number).await?;
    if order.is_success() {
        if let Some(order_data) = order.data {
            println!("Order Success! TrxID: {:?}", order_data.trxid);
        }
    } else {
        println!("Order Failed: {}", order.message);
    }
    */

    // -------------------------------------------------------------------------
    // 3. Check Order Status
    // -------------------------------------------------------------------------
    println!("\n=== 3. Check Order Status ===");
    let trx_id = "TRX123456789";

    println!(
        "(Status check skipped: uncomment below to check {})",
        trx_id
    );
    /*
    let status = client.status_order_prepaid(trx_id, None).await?;
    if status.is_success() {
        if let Some(items) = status.data {
            for item in items {
                println!("Status: {:?}", item.status);
                println!("SN: {:?}", item.sn);
            }
        }
    } else {
        println!("Check Status Failed: {}", status.message);
    }
    */

    Ok(())
}
