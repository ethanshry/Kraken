// Maybe look into this for ideas for safety
// https://github.com/babariviere/port_scanner-rs/blob/master/src/lib.rs
use async_std::prelude::*;
use async_std::stream;
use log::{error, info, warn};
use pnet::{datalink, ipnetwork::IpNetwork};
use std::convert::TryInto;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};

pub async fn scan_network_for_machines(port: u16) -> Vec<String> {
    let mut subnet_addr: Option<[u8; 3]> = None;
    'ifaces: for iface in datalink::interfaces() {
        println!("{:?}", iface.ips);
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

                    //subnet_addr = Some(addr.ip().octets()[0..2].try_into().unwrap());
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
            for x in 30..70 {
                addrs_to_scan.push(format!("{}.{}.{}.{}", subnet[0], subnet[1], subnet[2], x));
                /*addrs_to_scan.push(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(subnet[0], subnet[1], subnet[2], x)),
                    port,
                ));*/
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
            /*
            let mut items = addrs_to_scan.into_iter().map(|addr| async move {
                info!("Scanning network address {}", addr);

                let client = reqwest::Client::new();

                (addr, client
                    .get(&format!("http://{}:{}/ping", addr, port))
                    .send().await)
            });

            let mut items = items.into_iter().map(|i| async { i.await });

            let mut items = items.into_iter().filter_map(|i| {
                match i {
                    (addr, Ok(_)) => Some(addr),
                    (addr, Err(_)) => None,
                }
            })
            */
            /*
            let mut stream = stream::from_iter(addrs_to_scan.into_iter().filter_map(|addr| async {
                info!("Scanning network address {}", addr);
                let client = reqwest::Client::new();

                let response = client
                    .get(&format!("http://{}:{}/ping", addr, port))
                    .send()
                    .await;
                match response {
                    Ok(_) => true,
                    Err(_) => false,
                }
            });*/
            /*
            stream = stream.filter(|addr| async {
                info!("Scanning network address {}", addr);
                let client = reqwest::Client::new();

                let response = client
                    .get(&format!("http://{}:{}/ping", addr, port))
                    .send()
                    .await;
                match response {
                    Ok(_) => true,
                    Err(_) => false,
                }
            });*/

            //while let n = stream.next().await {
            //    println!("{:?}", n);
            //}
            //return vec![];
            /*
            let addrs: Vec<String> = addrs_to_scan
                .into_iter()
                .filter(|addr| async {
                    info!("Scanning network address {}", addr);
                    let client = reqwest::Client::new();

                    let response = client
                        .get(&format!("http://{}:{}/ping", addr, port))
                        .send()
                        .await;
                    match response {
                        Ok(_) => true,
                        Err(_) => false,
                    }
                    /*match reqwest::blocking::get(&format!("http://{}:{}/ping", addr, port)) {
                        Ok(_) => true,
                        Err(_) => false,
                    }*/
                    /*match TcpStream::connect_timeout(addr, std::time::Duration::new(0, 100000000)) {
                        Ok(s) => true,
                        Err(s) => false,
                    }*/
                })
                .collect();
            info!("Finished scanning, {} devices detected", addrs.len());
            info!("{:?}", addrs);

            return addrs;
            */
            /*.into_iter()
            .map(|socket_addr| {
                return socket_addr.ip();
            })
            .collect()*/
        }
    }
}

pub fn find_orchestrator_on_lan() -> Option<IpAddr> {
    return None;
}
