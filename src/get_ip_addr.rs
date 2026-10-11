use local_ip_address::list_afinet_netifas;
use std::{error::Error, fmt, net::IpAddr};

pub trait IPAddr {
    fn get_global_ip_addr(&self, interface: &str) -> Result<IpAddr, Box<dyn Error>>;
}

pub trait NetworkInterfaces {
    fn list(&self) -> Result<Vec<(String, IpAddr)>, Box<dyn Error>>;
}

pub struct SystemNetworkInterfaces;
impl NetworkInterfaces for SystemNetworkInterfaces {
    fn list(&self) -> Result<Vec<(String, IpAddr)>, Box<dyn Error>> {
        list_afinet_netifas().map_err(|error| Box::new(error) as Box<dyn Error>)
    }
}

pub struct LocalIPAddressClient<N: NetworkInterfaces> {
    network_interfaces: N,
}
impl<N: NetworkInterfaces> LocalIPAddressClient<N> {
    pub fn new(network_interfaces: N) -> Self {
        Self { network_interfaces }
    }
}
impl<N: NetworkInterfaces> IPAddr for LocalIPAddressClient<N> {
    fn get_global_ip_addr(&self, interface: &str) -> Result<IpAddr, Box<dyn Error>> {
        for (name, ip) in self.network_interfaces.list()? {
            if name == interface && ip.is_global() {
                return Ok(ip);
            }
        }
        Err(Box::new(NoGlobalIp))
    }
}

#[derive(Debug)]
struct NoGlobalIp;
impl fmt::Display for NoGlobalIp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Keep the original error text; callers currently ignore this error.
        write!(f, "Mock network error")
    }
}
impl Error for NoGlobalIp {}

#[cfg(test)]
mod tests {
    use super::*;
    struct FakeInterfaces(Vec<(&'static str, &'static str)>);
    impl NetworkInterfaces for FakeInterfaces {
        fn list(&self) -> Result<Vec<(String, IpAddr)>, Box<dyn Error>> {
            Ok(self
                .0
                .iter()
                .map(|(name, ip)| (name.to_string(), ip.parse().unwrap()))
                .collect())
        }
    }

    #[test]
    fn chooses_first_global_ip_on_exact_interface_without_family_filter() {
        let client = LocalIPAddressClient::new(FakeInterfaces(vec![
            ("eth01", "8.8.8.8"),
            ("eth0", "127.0.0.1"),
            ("eth0", "192.168.1.1"),
            ("eth0", "fe80::1"),
            ("eth0", "2606:4700:4700::1111"),
            ("eth0", "1.1.1.1"),
        ]));
        assert_eq!(
            client.get_global_ip_addr("eth0").unwrap(),
            "2606:4700:4700::1111".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn empty_missing_and_non_global_interfaces_are_errors() {
        for interfaces in [
            vec![],
            vec![("other", "8.8.8.8")],
            vec![("eth0", "10.0.0.1")],
        ] {
            assert_eq!(
                LocalIPAddressClient::new(FakeInterfaces(interfaces))
                    .get_global_ip_addr("eth0")
                    .unwrap_err()
                    .to_string(),
                "Mock network error"
            );
        }
    }

    #[test]
    fn enumeration_error_is_propagated() {
        struct FailingInterfaces;
        impl NetworkInterfaces for FailingInterfaces {
            fn list(&self) -> Result<Vec<(String, IpAddr)>, Box<dyn Error>> {
                Err(Box::new(std::io::Error::other("enumeration failed")))
            }
        }
        assert_eq!(
            LocalIPAddressClient::new(FailingInterfaces)
                .get_global_ip_addr("eth0")
                .unwrap_err()
                .to_string(),
            "enumeration failed"
        );
    }
}
