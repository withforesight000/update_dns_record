#![feature(ip)]
use std::process::exit;

use clap::Parser;
use get_ip_addr::{IPAddr, LocalIPAddressClient};
use http_client::ReqwestClient;

mod cloudflare_client;
mod errors;
mod get_ip_addr;
mod http_client;
mod logger;

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

    let ip_addr_client = get_ip_addr::IPAddrClient::new(LocalIPAddressClient::new());
    if args.v4_int.is_empty() && args.v6_int.is_empty() {
        eprintln!("At least one of v4_int or v6_int must be set");
        exit(1);
    }

    let v4_ip = if !args.v4_int.is_empty() {
        let v4_ip = ip_addr_client.client.get_global_ip_addr(&args.v4_int);
        match v4_ip {
            Ok(ip) => Some(ip),
            Err(_) => None,
        }
    } else {
        None
    };

    let v6_ip = if !args.v6_int.is_empty() {
        let v6_ip = ip_addr_client.client.get_global_ip_addr(&args.v6_int);
        match v6_ip {
            Ok(ip) => Some(ip),
            Err(_) => None,
        }
    } else {
        None
    };

    let mut logger = logger::new();

    if let Some(v4_ip) = v4_ip {
        logger.log_info(
            format!(
                "going to update A record to {} based on interface: {}",
                v4_ip, &args.v4_int
            )
            .as_str(),
        );
    }
    if let Some(v6_ip) = v6_ip {
        logger.log_info(
            format!(
                "going to update AAAA record to {} based on interface: {}",
                v6_ip, &args.v6_int
            )
            .as_str(),
        );
    }

    if args.dry_run {
        return;
    }

    let client = cloudflare_client::CloudflareClient::new(
        ReqwestClient::new(),
        args.api_token,
        "https://api.cloudflare.com/client/v4".to_string(),
        args.zone_id,
    );
    let dns_records = client.get_dns_records().unwrap_or_else(|error| {
        logger.log_error(format!("{}", error).as_str());
        exit(1);
    });

    let target_dns_records = cloudflare_client::filter_by_record_name(dns_records, &args.record_name)
        .unwrap_or_else(|error| {
            logger.log_error(format!("{}", error).as_str());
            exit(1);
        });

    if let Some(v4_ip) = v4_ip {
        let a_record = cloudflare_client::filter_by_record_type(&target_dns_records, "A")
        .unwrap_or_else(|err| {
            logger.log_error(format!("Failed to find A record: {}", err).as_str());
            exit(1);
        });

        let a_record_id = a_record.get("id").unwrap_or_else(|| {
            logger.log_error("Failed to find content");
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
        logger.log_info(format!("updated A record to {:?}", v4_ip).as_str());
    }

    if let Some(v6_ip) = v6_ip {
        let aaaa_record = cloudflare_client::filter_by_record_type(&target_dns_records, "AAAA")
        .unwrap_or_else(|err| {
            logger.log_error(format!("Failed to find A record: {}", err).as_str());
            exit(1);
        });

        let aaaa_record_id = aaaa_record.get("id").unwrap_or_else(|| {
            logger.log_error("Failed to find content");
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
        logger.log_info(format!("updated AAAA record to {:?}", v6_ip).as_str());
    }
}
