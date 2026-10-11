use local_ip_address::list_afinet_netifas;
use std::{error::Error, fmt, net::IpAddr};

#[derive(Clone, Copy)]
pub enum AddressFamily {
    V4,
    V6,
}
impl AddressFamily {
    pub fn matches(self, ip: IpAddr) -> bool {
        match self {
            Self::V4 => ip.is_ipv4(),
            Self::V6 => ip.is_ipv6(),
        }
    }
}

pub trait IPAddr {
    fn get_global_ip_addr(
        &self,
        interface: &str,
        family: AddressFamily,
    ) -> Result<IpAddr, Box<dyn Error>>;
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
    fn get_global_ip_addr(
        &self,
        interface: &str,
        family: AddressFamily,
    ) -> Result<IpAddr, Box<dyn Error>> {
        for (name, ip) in self.network_interfaces.list()? {
            if name == interface && ip.is_global() && family.matches(ip) {
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
        write!(f, "No global IP address of the requested family found")
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
    fn chooses_first_global_ip_of_requested_family_on_exact_interface() {
        let client = LocalIPAddressClient::new(FakeInterfaces(vec![
            ("eth01", "8.8.8.8"),
            ("eth0", "127.0.0.1"),
            ("eth0", "192.168.1.1"),
            ("eth0", "fe80::1"),
            ("eth0", "2606:4700:4700::1111"),
            ("eth0", "1.1.1.1"),
        ]));
        assert_eq!(
            client
                .get_global_ip_addr("eth0", AddressFamily::V6)
                .unwrap(),
            "2606:4700:4700::1111".parse::<IpAddr>().unwrap()
        );
        assert_eq!(
            client
                .get_global_ip_addr("eth0", AddressFamily::V4)
                .unwrap(),
            "1.1.1.1".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn empty_missing_and_non_global_interfaces_are_errors() {
        for interfaces in [
            vec![],
            vec![("other", "8.8.8.8")],
            vec![("eth0", "10.0.0.1")],
            vec![("eth0", "2606:4700:4700::1111")],
        ] {
            assert_eq!(
                LocalIPAddressClient::new(FakeInterfaces(interfaces))
                    .get_global_ip_addr("eth0", AddressFamily::V4)
                    .unwrap_err()
                    .to_string(),
                "No global IP address of the requested family found"
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
                .get_global_ip_addr("eth0", AddressFamily::V4)
                .unwrap_err()
                .to_string(),
            "enumeration failed"
        );
    }
}
