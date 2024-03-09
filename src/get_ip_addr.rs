use local_ip_address::list_afinet_netifas;

use crate::http_client::MockNetworkError;
pub trait IPAddr {
    fn get_global_ip_addr(&self, interface: &str) -> Result<std::net::IpAddr, Box<dyn std::error::Error>>;
}

pub struct IPAddrClient<T: IPAddr> {
    pub client: T
}

impl<T: IPAddr> IPAddrClient<T> {
    pub fn new(client: T) -> IPAddrClient<T> {
        IPAddrClient {
            client,
        }
    }
}

pub struct LocalIPAddressClient;

impl LocalIPAddressClient {
    pub fn new() -> LocalIPAddressClient {
        LocalIPAddressClient {}
    }
}

impl IPAddr for LocalIPAddressClient {
        fn get_global_ip_addr(&self, interface: &str) -> Result<std::net::IpAddr, Box<dyn std::error::Error>> {
        let network_interfaces = list_afinet_netifas();
        if let Err(e) = network_interfaces {
            return Err(Box::new(e));
        }
        let network_interfaces = network_interfaces.unwrap();

        for (name, ip) in network_interfaces.iter() {
            if name == interface && ip.is_global() {
                return Ok(*ip)
            }
        }
        Err(Box::new(MockNetworkError))
    }
}
