#![feature(ip)]
use std::process::exit;

use clap::Parser;
use get_ip_addr::{LocalIPAddressClient, SystemNetworkInterfaces};
use http_client::ReqwestClient;

mod app;
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
    #[arg(
        short,
        long,
        required = true,
        env = "CLOUDFLARE_API_TOKEN",
        hide_env_values = true
    )]
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
    let ip = LocalIPAddressClient::new(SystemNetworkInterfaces);
    let result = app::run(
        &args,
        &ip,
        || {
            cloudflare_client::CloudflareClient::new(
                ReqwestClient::new(),
                args.api_token.clone(),
                "https://api.cloudflare.com/client/v4".to_string(),
                args.zone_id.clone(),
            )
        },
        logger::new,
    );
    if let Err(error) = result {
        if let app::AppError::Stderr(message) = error {
            eprintln!("{}", message);
        }
        exit(1);
    }
}
