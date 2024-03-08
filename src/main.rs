use clap::Parser;
use http_client::ReqwestClient;

use std::process::exit;

mod cloudflare_client;
mod http_client;
mod errors;

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

    #[arg(long, default_value = "", env = "CLOUDFLARE_A_RECORD_VAULE")]
    a_record_value: String,

    #[arg(long, default_value = "", env = "CLOUDFLARE_AAAA_RECORD_VAULE")]
    aaaa_record_value: String,
}

fn main() {
    let args = Args::parse();

    let client = cloudflare_client::CloudflareClient::new(
        ReqwestClient::new(),
        args.api_token,
        "https://localhost".to_string(),
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

    if !args.a_record_value.is_empty() {
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
            .update_record(a_record_id, &serde_json::Value::String(args.a_record_value))
            .unwrap_or_else(|error| {
                eprintln!("{}", error);
                exit(1);
            });
    }

    if !args.aaaa_record_value.is_empty() {
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
                &serde_json::Value::String(args.aaaa_record_value),
            )
            .unwrap_or_else(|error| {
                eprintln!("{}", error);
                exit(1);
            })
    }
}
