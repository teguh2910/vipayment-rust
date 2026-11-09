//! Command-line interface for the VIPayment API.

use std::env;
use vipayment::{ApiResponse, VipaymentClient};

fn print_usage() {
    println!(
        r#"VIPayment CLI

USAGE:
    vipayment <SUBCOMMAND> [OPTIONS]

ENVIRONMENT VARIABLES:
    VIPAYMENT_API_ID     Your VIPayment API ID
    VIPAYMENT_API_KEY    Your VIPayment API Key

SUBCOMMANDS:
    profile                                 Get account balance and profile details
    prepaid-services [type] [value]         List prepaid catalog services
    prepaid-order <service> <target_no>     Place a prepaid order (e.g. PLN5 1234567890)
    prepaid-status <trx_id> [limit]         Check status of a prepaid transaction
    game-services [game] [status]           List game catalog services
    game-order <service> <user_id> [zone]   Place a game top-up order
    game-status <trx_id> [limit]            Check status of a game transaction
    help                                    Print this help message
"#
    );
}

fn get_client() -> VipaymentClient {
    let api_id = env::var("VIPAYMENT_API_ID").unwrap_or_else(|_| {
        eprintln!(
            "Warning: VIPAYMENT_API_ID environment variable not set, using default placeholder"
        );
        "your_api_id".to_string()
    });
    let api_key = env::var("VIPAYMENT_API_KEY").unwrap_or_else(|_| {
        eprintln!(
            "Warning: VIPAYMENT_API_KEY environment variable not set, using default placeholder"
        );
        "your_api_key".to_string()
    });

    VipaymentClient::new(api_id, api_key)
}

fn print_response<T: serde::Serialize>(resp: &ApiResponse<T>) {
    match serde_json::to_string_pretty(resp) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Failed to format response JSON: {}", e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let subcommand = args[1].as_str();

    match subcommand {
        "help" | "-h" | "--help" => {
            print_usage();
        }

        "profile" => {
            let client = get_client();
            let res = client.profile().await?;
            print_response(&res);
        }

        "prepaid-services" => {
            let client = get_client();
            let filter_type = args.get(2).map(|s| s.as_str());
            let filter_value = args.get(3).map(|s| s.as_str());
            let res = client.service_prepaid(filter_type, filter_value).await?;
            print_response(&res);
        }

        "prepaid-order" => {
            if args.len() < 4 {
                eprintln!("Error: Missing arguments.");
                eprintln!("Usage: vipayment prepaid-order <service_code> <target_no>");
                std::process::exit(1);
            }
            let client = get_client();
            let service = &args[2];
            let data_no = &args[3];
            let res = client.order_prepaid(service, data_no).await?;
            print_response(&res);
        }

        "prepaid-status" => {
            if args.len() < 3 {
                eprintln!("Error: Missing transaction ID.");
                eprintln!("Usage: vipayment prepaid-status <trx_id> [limit]");
                std::process::exit(1);
            }
            let client = get_client();
            let trx_id = &args[2];
            let limit = args.get(3).and_then(|s| s.parse::<u32>().ok());
            let res = client.status_order_prepaid(trx_id, limit).await?;
            print_response(&res);
        }

        "game-services" => {
            let client = get_client();
            let filter_game = args.get(2).map(|s| s.as_str());
            let filter_status = args.get(3).map(|s| s.as_str());
            let res = client.service_game(filter_game, filter_status).await?;
            print_response(&res);
        }

        "game-order" => {
            if args.len() < 4 {
                eprintln!("Error: Missing arguments.");
                eprintln!("Usage: vipayment game-order <service_code> <user_id> [zone_id]");
                std::process::exit(1);
            }
            let client = get_client();
            let service = &args[2];
            let user_id = &args[3];
            let zone_id = args.get(4).map(|s| s.as_str());
            let res = client.order_game(service, user_id, zone_id).await?;
            print_response(&res);
        }

        "game-status" => {
            if args.len() < 3 {
                eprintln!("Error: Missing transaction ID.");
                eprintln!("Usage: vipayment game-status <trx_id> [limit]");
                std::process::exit(1);
            }
            let client = get_client();
            let trx_id = &args[2];
            let limit = args.get(3).and_then(|s| s.parse::<u32>().ok());
            let res = client.status_order_game(trx_id, limit).await?;
            print_response(&res);
        }

        unknown => {
            eprintln!("Unknown subcommand: {}", unknown);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}
