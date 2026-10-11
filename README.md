# Update DNS Record

## Overview
`update_dns_record` is a Rust CLI utility that keeps a Cloudflare DNS record in sync with the global IP address assigned to a local network interface. The application discovers a global IP address on each configured interface and updates the matching A and AAAA records in a Cloudflare zone using the Cloudflare v4 API.

## Features
- Resolves the global IP address for a specified local network interface using `local-ip-address`.
- Updates Cloudflare DNS A and AAAA records over HTTPS with a bearer API token.
- Supports independent IPv4 and IPv6 configuration, making either optional.
- Provides a `--dry-run` flag to preview actions without contacting Cloudflare.
- Logs to stdout when attached to a terminal and falls back to syslog for unattended environments.

## Prerequisites
- To build from source: the pinned Rust nightly toolchain (see `rust-toolchain.toml`).
- To use release binaries: GNU/Linux with glibc 2.35 or newer (Ubuntu 22.04 or newer), on amd64 or arm64.
- An up-to-date system CA certificate store (for example, the `ca-certificates` package on Ubuntu). Starting with 1.1.0, reqwest's Rustls backend validates HTTPS certificates using the platform trust store.
- A terminal for stdout logging, or a local syslog service for unattended execution.
- Cloudflare API token with permissions to read and edit DNS records in the target zone.

## Installation
Download the archive for your architecture and `SHA256SUMS` from
[GitHub Releases](https://github.com/withforesight000/update_dns_record/releases).
Archives are named `update_dns_record-<version>-linux-amd64.tar.gz` and
`update_dns_record-<version>-linux-arm64.tar.gz`. Each contains the executable
`update_dns_record`.

For example, with both archives and `SHA256SUMS` in the current directory:

```bash
sha256sum --check SHA256SUMS
tar -xzf update_dns_record-1.1.0-linux-arm64.tar.gz
./update_dns_record --version
```

Use the amd64 archive on x86-64 systems. Binaries are built and executed on
Ubuntu 22.04 runners; older glibc environments, musl/Alpine, macOS, and Windows
are not supported by these release archives.

Alternatively, clone the repository and build the binary with Cargo:

```bash
cargo build --release --locked
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
Help output lists the API token environment variable name but hides its value.

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
Run the regression suite and code checks with the pinned toolchain:

```bash
cargo test --locked --all-targets
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

The update workflow accepts injected IP discovery, DNS operations, and logger
factories. NIC enumeration is also injected. Tests use in-memory responses and
fake dependencies; they do not contact Cloudflare, change DNS, or require syslog.

GitHub Actions runs these checks on native Linux amd64 and arm64 runners for
branch pushes and pull requests. A numeric version tag such as `1.0.0` must
match the Cargo package version. Tag pushes run checks and release builds,
verify both archives, upload them with `SHA256SUMS` to a draft, and publish the
complete release. Separate native jobs download and execute the published
archives. A failed upload or draft verification leaves the release unpublished;
a rerun can resume the same draft. Published releases are never overwritten by
the workflow. Post-publication fixes use a new patch version.

The arm64 CI runner overrides the existing cross-compilation linker setting
with `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=cc` for native builds.
The existing Drone configuration remains available.

## Behavior and limitations

- Each configured interface uses its first global IP of the requested family
  in the enumeration order: IPv4 for A and IPv6 for AAAA. Both options can
  select the same dual-stack interface.
- IP discovery failures are ignored. Without any discovered IP, a normal run
  still fetches and filters DNS records but does not patch them.
- A is updated before AAAA. If the second update fails, the first update remains.
- Malformed API fields and an unavailable syslog service can cause a panic.
- Dry-run still discovers IPs and initializes logging, but never constructs the
  DNS client or makes API calls.

## License
This project is provided as-is without an explicit license. Consult the repository owner before production use.
