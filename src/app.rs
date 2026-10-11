use crate::cloudflare_client::{filter_by_record_name, filter_by_record_type, DnsClient};
use crate::get_ip_addr::{AddressFamily, IPAddr};
use crate::logger::Logger;
use crate::Args;

#[derive(Debug, PartialEq)]
pub enum AppError {
    Stderr(String),
    Logged,
}

pub fn run<I, D, DF, LF>(
    args: &Args,
    ip: &I,
    dns_factory: DF,
    logger_factory: LF,
) -> Result<(), AppError>
where
    I: IPAddr,
    D: DnsClient,
    DF: FnOnce() -> D,
    LF: FnOnce() -> Box<dyn Logger>,
{
    if args.v4_int.is_empty() && args.v6_int.is_empty() {
        return Err(AppError::Stderr(
            "At least one of v4_int or v6_int must be set".into(),
        ));
    }
    let mut discovery_errors = Vec::new();
    let addresses = [
        (&args.v4_int, "A", AddressFamily::V4),
        (&args.v6_int, "AAAA", AddressFamily::V6),
    ]
    .map(|(interface, record_type, family)| {
        let address = if interface.is_empty() {
            None
        } else {
            match ip.get_global_ip_addr(interface, family) {
                Ok(address) => Some(address),
                Err(error) => {
                    discovery_errors.push(format!(
                        "Failed to discover {record_type} address on interface {interface}: {error}"
                    ));
                    None
                }
            }
        };
        (interface, record_type, address)
    });
    let mut logger = logger_factory();
    for error in &discovery_errors {
        logger.log_error(error);
    }
    let discovery_result = if discovery_errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Logged)
    };
    for (interface, record_type, address) in &addresses {
        if let Some(address) = address {
            logger.log_info(&format!(
                "going to update {} record to {} based on interface: {}",
                record_type, address, interface
            ));
        }
    }
    if args.dry_run || addresses.iter().all(|(_, _, address)| address.is_none()) {
        return discovery_result;
    }

    let dns = dns_factory();
    let records = dns.get_dns_records().map_err(|error| {
        logger.log_error(&error.to_string());
        AppError::Logged
    })?;
    let records = filter_by_record_name(records, &args.record_name).map_err(|error| {
        logger.log_error(&error.to_string());
        AppError::Logged
    })?;
    for (_, record_type, address) in addresses {
        if let Some(address) = address {
            let record = filter_by_record_type(&records, record_type).map_err(|error| {
                // Preserve the existing message for both record types.
                logger.log_error(&format!("Failed to find A record: {}", error));
                AppError::Logged
            })?;
            let id = record.get("id").ok_or_else(|| {
                logger.log_error("Failed to find content");
                AppError::Logged
            })?;
            dns.update_record(
                id,
                &serde_json::Value::String(address.to_string()),
                &args.record_name,
                record_type,
            )
            .map_err(|error| AppError::Stderr(error.to_string()))?;
            logger.log_info(&format!("updated {} record to {:?}", record_type, address));
        }
    }
    discovery_result
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
