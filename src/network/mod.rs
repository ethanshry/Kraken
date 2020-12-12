//! Functions related to the local network
//! - subnet scanning
//! - healthchecks
//! - etc

// Maybe look into this for ideas for safety
// https://github.com/babariviere/port_scanner-rs/blob/master/src/lib.rs
use log::info;
use pnet::{datalink, ipnetwork::IpNetwork};

// Constants to restrict subnet to search through, speeds up search significantly
const MIN_SUBNET_ADDR: u8 = 30;
const MAX_SUBNET_ADDR: u8 = 70;

pub async fn scan_network_for_machines(port: u16) -> Vec<String> {
    let mut subnet_addr: Option<[u8; 3]> = None;
    'ifaces: for iface in datalink::interfaces() {
        for ip in iface.ips {
            if let IpNetwork::V4(addr) = ip {
                if addr.ip().to_string() != "127.0.0.1" {
                    // we are not looking at localhost, so use it
                    // TODO do something besides assume 24 bit prefix
                    let mut subnet_addr_vec: [u8; 3] = [0, 0, 0];

                    let octets: [u8; 4] = addr.ip().octets();

                    subnet_addr_vec[..3].clone_from_slice(&octets[..3]);

                    subnet_addr = Some(subnet_addr_vec);

                    break 'ifaces;
                }
            }
        }
    }

    match subnet_addr {
        None => return vec![],
        Some(subnet) => {
            let mut addrs_to_scan: Vec<String> = vec![];
            for x in MIN_SUBNET_ADDR..MAX_SUBNET_ADDR {
                addrs_to_scan.push(format!("{}.{}.{}.{}", subnet[0], subnet[1], subnet[2], x));
            }
            // TODO maybe look into https://github.com/rayon-rs/rayon to make this better : see #45
            let mut open_addrs = vec![];
            for addr in addrs_to_scan.iter() {
                info!("Scanning network address {}", addr);
                if reqwest::Client::new()
                    .get(&format!("http://{}:{}/ping", addr, port))
                    .timeout(std::time::Duration::from_millis(500))
                    .send()
                    .await
                    .is_ok()
                {
                    open_addrs.push(addr.to_string());
                }
            }

            info!("{:?}", open_addrs);
            return open_addrs;
        }
    }
}

pub async fn find_orchestrator_on_lan() -> Option<String> {
    // TODO drive port from ENV or config or something
    let machines = scan_network_for_machines(8000).await; // Look for Rocket server
    if !machines.is_empty() {
        return Some(machines[0].to_string());
    }
    return None;
}

/// Tries to hit a url, and returns success (true) or faliure (false)
pub async fn healthcheck(url: &str) -> bool {
    reqwest::Client::new()
        .get(url)
        .timeout(std::time::Duration::new(1, 0))
        .send()
        .await
        .is_ok()
}

/// Continually retries healthcheck until is accepted,
/// Will try infinitely unless given a retry limit
/// returns success (true) or faliure (false)
pub async fn wait_for_good_healthcheck(url: &str, retry_count: Option<u16>) -> bool {
    match retry_count {
        Some(i) => {
            for _ in 0..i {
                if let true = healthcheck(url).await {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            return false;
        }
        None => loop {
            if let true = healthcheck(url).await {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        },
    }
}
