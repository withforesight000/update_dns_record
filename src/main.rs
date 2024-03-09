#![feature(ip)]
use clap::Parser;
use get_ip_addr::{IPAddr, LocalIPAddressClient};
use http_client::ReqwestClient;

use std::process::exit;

mod cloudflare_client;
mod errors;
mod get_ip_addr;
mod http_client;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// API token for Cloudflare
    #[arg(short, long, required = true, env = "CLOUDFLARE_API_TOKEN")]
    api_token: String,
    /// Zone ID for Cloudflare
    #[arg(short, long, required = true, env = "CLOUDFLARE_ZONE_ID")]
    zone_id: String,

    #[arg(short, long, required = true, env = "CLOUDFLARE_RECORD_NAME")]
    record_name: String,

    #[arg(long, default_value = "", env = "CLOUDFLARE_V4_INT")]
    v4_int: String,

    #[arg(long, default_value = "", env = "CLOUDFLARE_V6_INT")]
    v6_int: String,

    #[arg(long, env = "CLOUDFLARE_DRY_RUN")]
    dry_run: bool,
}

fn main() {
    let args = Args::parse();

    let v4_int = args.v4_int;
    let v6_int = args.v6_int;

    let ip_addr_client = get_ip_addr::IPAddrClient::new(LocalIPAddressClient::new());
    let v4_ip = ip_addr_client
        .client
        .get_global_ip_addr(&v4_int)
        .unwrap_or_else(|error| {
            eprintln!("{}", error);
            exit(1);
        });
    let v6_ip = ip_addr_client
        .client
        .get_global_ip_addr(&v6_int)
        .unwrap_or_else(|error| {
            eprintln!("{}", error);
            exit(1);
        });

    if args.dry_run {
        println!("Would update A record to {}", v4_ip);
        println!("Would update AAAA record to {}", v6_ip);
        return;
    }

    println!("Would update A record to {} from {}", v4_ip, &v4_int);
    println!("Would update AAAA record to {} from {}", v6_ip, &v6_int);

    let client = cloudflare_client::CloudflareClient::new(
        ReqwestClient::new(),
        args.api_token,
        "https://api.cloudflare.com/client/v4".to_string(),
        args.zone_id,
    );
    let dns_records = client.get_dns_records().unwrap_or_else(|error| {
        eprintln!("{}", error);
        exit(1);
    });

    let target_dns_records = client
        .find_record_ids(dns_records, &args.record_name)
        .unwrap_or_else(|error| {
            eprintln!("{}", error);
            exit(1);
        });

    if !v4_int.is_empty() {
        let a_record = target_dns_records
            .iter()
            .find(|record| record["type"].as_str().unwrap() == "A")
            .unwrap_or_else(|| {
                eprintln!("Failed to find A record");
                exit(1);
            });

        let a_record_id = a_record.get("id").unwrap_or_else(|| {
            eprintln!("Failed to find content");
            exit(1);
        });

        _ = client
            .update_record(
                a_record_id,
                &serde_json::Value::String(v4_ip.to_string()),
                &args.record_name,
                "A",
            )
            .unwrap_or_else(|error| {
                eprintln!("{}", error);
                exit(1);
            });
        println!("updated A record to {:?}", v4_ip.to_string());
    }

    if !v6_int.is_empty() {
        let aaaa_record = target_dns_records
            .iter()
            .find(|record| record["type"].as_str().unwrap() == "AAAA")
            .unwrap_or_else(|| {
                eprintln!("Failed to find AAAA record");
                exit(1);
            });

        let aaaa_record_id = aaaa_record.get("id").unwrap_or_else(|| {
            eprintln!("Failed to find content");
            exit(1);
        });

        _ = client
            .update_record(
                aaaa_record_id,
                &serde_json::Value::String(v6_ip.to_string()),
                &args.record_name,
                "AAAA",
            )
            .unwrap_or_else(|error| {
                eprintln!("{}", error);
                exit(1);
            });
        println!("updated AAAA record to {:?}", v6_ip.to_string());
    }
}
