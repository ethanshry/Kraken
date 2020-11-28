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
        //println!("{:?}", iface.ips);
        for ip in iface.ips {
            if let IpNetwork::V4(addr) = ip {
                if addr.ip().to_string() != "127.0.0.1" {
                    // we are not looking at localhost, so use it
                    // TODO do something besides assume 24 bit prefix
                    let mut subnet_addr_vec: [u8; 3] = [0, 0, 0];

                    let octets: [u8; 4] = addr.ip().octets();

                    for x in 0..3 {
                        subnet_addr_vec[x] = octets[x];
                    }

                    subnet_addr = Some(subnet_addr_vec);

                    break 'ifaces;
                }
            }
            break;
        }
    }

    match subnet_addr {
        None => return vec![],
        Some(subnet) => {
            let mut addrs_to_scan: Vec<String> = vec![];
            for x in MIN_SUBNET_ADDR..MAX_SUBNET_ADDR {
                addrs_to_scan.push(format!("{}.{}.{}.{}", subnet[0], subnet[1], subnet[2], x));
            }
            // TODO maybe look into https://github.com/rayon-rs/rayon to make this better
            let mut open_addrs = vec![];
            for addr in addrs_to_scan.iter() {
                info!("Scanning network address {}", addr);
                match reqwest::Client::new()
                    .get(&format!("http://{}:{}/ping", addr, port))
                    .timeout(std::time::Duration::new(0, 500000000))
                    .send()
                    .await
                {
                    Ok(_) => open_addrs.push(addr.to_string()),
                    Err(_) => {}
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
    if machines.len() != 0 {
        return Some(machines.get(0).unwrap().to_string());
    }
    return None;
}