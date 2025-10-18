# Update DNS Record

## Overview
`update_dns_record` is a Rust CLI utility that keeps a Cloudflare DNS record in sync with the global IP address assigned to a local network interface. The application discovers the interface's public IPv4 and/or IPv6 address and updates the matching A and AAAA records in a Cloudflare zone using the Cloudflare v4 API.

## Features
- Resolves the global IP address for a specified local network interface using `local-ip-address`.
- Updates Cloudflare DNS A and AAAA records over HTTPS with a bearer API token.
- Supports independent IPv4 and IPv6 configuration, making either optional.
- Provides a `--dry-run` flag to preview actions without contacting Cloudflare.
- Logs to stdout when attached to a terminal and falls back to syslog for unattended environments.

## Prerequisites
- Rust nightly toolchain (see `rust-toolchain.toml`).
- Cloudflare API token with permissions to read and edit DNS records in the target zone.

## Installation
Clone the repository and build the binary with Cargo:

```bash
cargo build --release
```

The compiled binary will be located at `target/release/update_dns_record`.

## Configuration
You can configure the CLI through command-line flags or the corresponding environment variables:

| Flag | Environment variable | Description |
| ---- | -------------------- | ----------- |
| `--api-token` | `CLOUDFLARE_API_TOKEN` | Cloudflare API token used for authorization. |
| `--zone-id` | `CLOUDFLARE_ZONE_ID` | Identifier of the zone containing the DNS record. |
| `--record-name` | `CLOUDFLARE_RECORD_NAME` | Fully qualified domain name of the DNS record to update. |
| `--v4-int` | `CLOUDFLARE_V4_INT` | Network interface whose global IPv4 address should populate the A record. Leave empty to skip A record updates. |
| `--v6-int` | `CLOUDFLARE_V6_INT` | Network interface whose global IPv6 address should populate the AAAA record. Leave empty to skip AAAA record updates. |
| `--dry-run` | `CLOUDFLARE_DRY_RUN` | When set, log planned updates without sending API requests. |

At least one of `--v4-int` or `--v6-int` must be supplied.

## Usage
To update both A and AAAA records using environment variables:

```bash
export CLOUDFLARE_API_TOKEN="cf_api_token"
export CLOUDFLARE_ZONE_ID="cf_zone_id"
export CLOUDFLARE_RECORD_NAME="example.com"
export CLOUDFLARE_V4_INT="eth0"
export CLOUDFLARE_V6_INT="eth0"

cargo run --release -- --dry-run
```

Remove `--dry-run` to apply the changes. Use `cargo run -- --help` to view the full CLI reference.

## Development
Run the test suite to validate the client and helper utilities:

```bash
cargo test
```

## License
This project is provided as-is without an explicit license. Consult the repository owner before production use.
