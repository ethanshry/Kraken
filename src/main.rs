#![feature(proc_macro_hygiene, decl_macro, async_closure)]
#![allow(dead_code)]

#[macro_use]
extern crate juniper;
extern crate fs_extra;
extern crate rocket;
extern crate rocket_cors;
extern crate serde;
extern crate strum;
extern crate strum_macros;
extern crate tokio;

mod db;
mod docker;
mod file_utils;
mod git_utils;
mod gitapi;
mod model;
mod platform_executor;
mod rabbit;
mod routes;
mod schema;
mod testing;
mod utils;
mod worker;

use log::{error, info, warn};
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
    let rabbit_addr: String =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://localhost:5672".into());

    info!("{}", rabbit_addr);

    let system_uuid = utils::get_system_id();

    let mut node = GenericNode::new(&system_uuid, &rabbit_addr);

    let mut worker = platform_executor::worker::Worker::new();

    let node_mode = match platform_executor::worker::setup(&mut node, &mut worker).await {
        Ok(_) => NodeMode::WORKER,
        Err(e) => {
            warn!("{:?}", e);
            NodeMode::ORCHESTRATOR
        }
    };

    let mut orchestrator = platform_executor::orchestrator::Orchestrator::new();

    if node_mode == NodeMode::ORCHESTRATOR {
        match platform_executor::orchestrator::setup(&mut node, &mut orchestrator).await {
            Ok(_) => {}
            Err(e) => {
                error!("{:?}", e);
                panic!(
                    "Failed to create orchestrator, node cannot attach to or form platform. Exiting."
                )
            }
        }
    }

    testing::setup_experiment(&mut node, &mut orchestrator).await;

    loop {
        match node_mode {
            NodeMode::ORCHESTRATOR => {
                match platform_executor::orchestrator::execute(&orchestrator).await {
                    Ok(_) => {}
                    Err(faliure) => match faliure {
                        TaskFaliure::SigKill => {
                            panic!("Orchestrator indicated a critical execution faliure")
                        }
                    },
                }
            }
            NodeMode::WORKER => match platform_executor::worker::execute(&worker).await {
                Ok(_) => {}
                Err(faliure) => match faliure {
                    TaskFaliure::SigKill => panic!("Worker indicated a critical execution faliure"),
                },
            },
        }
        std::thread::sleep(std::time::Duration::new(0, 500000000));
    }

    #[allow(unreachable_code)]
    Ok(())
}
