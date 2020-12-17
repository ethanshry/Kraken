//! Functions related to the local network
//! - subnet scanning
//! - healthchecks
//! - etc

// Maybe look into this for ideas for safety
// https://github.com/babariviere/port_scanner-rs/blob/master/src/lib.rs
use futures::future;
use log::info;
use pnet::{datalink, ipnetwork::IpNetwork};

// Constants to restrict subnet to search through, speeds up search significantly
const MIN_SUBNET_ADDR: u8 = 0;
const MAX_SUBNET_ADDR: u8 = 255;

/// Gets the IPv4 LAN address for your device
/// Will not return localhost
/// # Examples
///
/// ```
/// let addr = get_lan_addr();
/// assert_ne!(Some("127.0.0.1", addr))
/// ```
pub fn get_lan_addr() -> Option<String> {
    let mut addr: Option<String> = None;
    'ifaces: for iface in datalink::interfaces() {
        for ip in iface.ips {
            if let IpNetwork::V4(a) = ip {
                if a.ip().to_string() != "127.0.0.1" {
                    // we are not looking at localhost, so use it
                    addr = Some(a.ip().to_string());
                    break 'ifaces;

                }
            }
        }
    }
    return addr;
}

/// Searches the local network for machines on the specified port
/// Returns a Vec of addresses which have the specified port open
/// # Arguments
///
/// * `port` - The port to search on
/// # Examples
///
/// ```
/// let devices = scan_network_for_machines(crate::utils::ROCKET_PORT_NO).await;
/// ```
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

            let mut open_addrs = vec![];
            // Shoutout to https://stackoverflow.com/questions/61481079/how-can-i-join-all-the-futures-in-a-vector-without-cancelling-on-failure-like-jo for this magic
            let open_addr_futures: Vec<_> = addrs_to_scan
                .iter()
                .map(|addr| async move {
                    match reqwest::Client::new()
                        .get(&format!("http://{}:{}/ping", addr, port))
                        .timeout(std::time::Duration::from_millis(500))
                        .send()
                        .await
                        .is_ok()
                    {
                        true => Ok(addr.to_string()),
                        false => Err(addr.to_string()),
                    }
                })
                .collect();

            let unpin_futs: Vec<_> = open_addr_futures.into_iter().map(Box::pin).collect();
            let mut futs = unpin_futs;

            while !futs.is_empty() {
                match future::select_all(futs).await {
                    (Ok(addr), _index, remaining) => {
                        info!("Addr! {}", addr);
                        open_addrs.push(addr.clone());
                        futs = remaining;
                    }
                    (Err(addr), _index, remaining) => {
                        info!("Nothing found at {}", addr);
                        futs = remaining;
                    }
                }
            }

            info!("{:?}", open_addrs);
            return open_addrs;
        }
    }
}

/// Looks for an orchestrator on the local network (i.e. an open Rocket.rs HTTP server)
/// Returns Some(addr) if one is found, and None otherwise

/// # Examples
///
/// ```
/// let orchestrator = find_orchestrator_on_lan().await;
/// assert_eq!(orchestrator, None);
/// ```
pub async fn find_orchestrator_on_lan() -> Option<String> {
    let machines = scan_network_for_machines(crate::utils::ROCKET_PORT_NO).await;
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
