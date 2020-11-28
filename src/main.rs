#![feature(proc_macro_hygiene, decl_macro, async_closure)]
#![allow(dead_code)]

#[macro_use]
extern crate juniper;
extern crate fs_extra;
extern crate queues;
extern crate rocket;
extern crate rocket_cors;
extern crate serde;
extern crate strum;
extern crate strum_macros;
extern crate tokio;

mod db;
mod deployment;
mod docker;
mod file_utils;
mod git_utils;
mod gitapi;
mod model;
mod network;
mod platform_executor;
mod rabbit;
mod routes;
mod schema;
mod testing;
mod utils;
mod worker;

use log::{error, info};
use platform_executor::{GenericNode, TaskFaliure};

#[derive(Debug, Clone, PartialEq)]
pub enum NodeMode {
    WORKER,
    ORCHESTRATOR,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let mut orchestrator_ip: Option<String> = network::find_orchestrator_on_lan().await;

    let node_mode = match &orchestrator_ip {
        Some(ip) => {
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
        orchestrator_ip.unwrap_or(String::from("localhost"))
    );
    //std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672".into());

    info!("{}", rabbit_addr);

    let system_uuid = utils::get_system_id();
    info!("{}", system_uuid);

    let mut node = GenericNode::new(&system_uuid, &rabbit_addr);
    info!("node established");

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

    testing::setup_experiment(&mut node, &mut orchestrator).await;

    loop {
        match node_mode {
            NodeMode::ORCHESTRATOR => {
                match platform_executor::orchestrator::execute(&node, &orchestrator).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        TaskFaliure::SigKill => {
                            panic!("Orchestrator indicated a critical execution faliure")
                        }
                    },
                };
                // An Orchestrator IS a worker, so do worker tasks too
                // This may cause problems later due to sharing of Node information? Not sure
                match platform_executor::worker::execute(&mut node, &mut worker).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        TaskFaliure::SigKill => {
                            panic!("Worker indicated a critical execution faliure")
                        }
                    },
                };
            }
            NodeMode::WORKER => {
                match platform_executor::worker::execute(&mut node, &mut worker).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        TaskFaliure::SigKill => {
                            panic!("Worker indicated a critical execution faliure")
                        }
                    },
                }
            }
        }
        std::thread::sleep(std::time::Duration::new(0, 500000000));
    }

    #[allow(unreachable_code)]
    Ok(())
}
