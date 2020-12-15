//! Kraken is a LAN-based, distributed, fault tolerant application deployment platform
//!
//! See CRATE_MANIFEST_DIR/documentation for more information

#![feature(proc_macro_hygiene, decl_macro, async_closure)]
#![allow(dead_code)]
#![warn(missing_docs, rust_2018_idioms, missing_debug_implementations, unused_extern_crates)]

#[macro_use]
extern crate juniper;
extern crate strum_macros;

mod api_routes;
mod db;
mod deployment;
mod docker;
mod file_utils;
mod git_utils;
mod github_api;
mod gql_model;
mod gql_schema;
mod network;
mod platform_executor;
mod rabbit;
mod testing;
mod utils;

use log::{error, info, warn};
use platform_executor::{ExecuteFaliure, GenericNode, NodeMode};

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let orchestrator_ip =
        match &std::env::var("SHOULD_SCAN_NETWORK").unwrap_or_else(|_| "YES".into())[..] {
            "NO" => {
                warn!("ENV is configured to skip network scan, this may not be desired behaviour");
                None
            }
            _ => network::find_orchestrator_on_lan().await,
        };

    let node_mode = match &orchestrator_ip {
        Some(_) => {
            info!("Orchestrator detected, starting node as worker");
            NodeMode::WORKER
        }
        None => {
            info!("No orchestrator detected, starting node as orchestrator");
            NodeMode::ORCHESTRATOR
        }
    };

    let rabbit_addr: String = format!(
        "amqp://{}:5672",
        orchestrator_ip.unwrap_or_else(|| String::from("localhost"))
    );

    info!("rabbit addr will be {}", rabbit_addr);

    let system_uuid = utils::get_system_id();

    let mut node = GenericNode::new(&system_uuid, &rabbit_addr);
    info!("node {} established", system_uuid);

    let mut worker = platform_executor::worker::Worker::new();

    let mut orchestrator = platform_executor::orchestrator::Orchestrator::new();

    if node_mode == NodeMode::ORCHESTRATOR {
        match platform_executor::orchestrator::setup(&mut node, &mut orchestrator).await {
            Ok(_) => {
                // Now that we have a platform, re-establish the worker
                worker = platform_executor::worker::Worker::new();
                platform_executor::worker::setup(&mut node, &mut worker)
                    .await
                    .unwrap();
            }
            Err(e) => {
                error!("{:?}", e);
                panic!(
                    "Failed to create orchestrator, node cannot attach to or form platform. Exiting."
                )
            }
        }
    } else {
        platform_executor::worker::setup(&mut node, &mut worker)
            .await
            .unwrap();
    }

    // TODO remove for final
    testing::setup_experiment(&mut node, &mut orchestrator).await;

    loop {
        match node_mode {
            NodeMode::ORCHESTRATOR => {
                match platform_executor::orchestrator::execute(&node, &orchestrator).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        ExecuteFaliure::SigKill => {
                            panic!("Orchestrator indicated a critical execution faliure")
                        }
                    },
                };
                // An Orchestrator IS a worker, so do worker tasks too
                // This may cause problems later due to sharing of Node information? Not sure
                match platform_executor::worker::execute(&mut node, &mut worker).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        ExecuteFaliure::SigKill => {
                            panic!("Worker indicated a critical execution faliure")
                        }
                    },
                };
            }
            NodeMode::WORKER => {
                match platform_executor::worker::execute(&mut node, &mut worker).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        ExecuteFaliure::SigKill => {
                            panic!("Worker indicated a critical execution faliure")
                        }
                    },
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[allow(unreachable_code)]
    Ok(())
}
