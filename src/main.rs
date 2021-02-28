//! Kraken is a LAN-based, distributed, fault tolerant application deployment platform
//!
//! See CRATE_MANIFEST_DIR/documentation for more information

#![feature(proc_macro_hygiene, decl_macro, async_closure)]
#![allow(dead_code)]
#![warn(
    missing_docs,
    rust_2018_idioms,
    missing_debug_implementations,
    unused_extern_crates
)]

#[macro_use]
extern crate juniper;

mod api_routes;
mod deployment;
mod docker;
mod gql_model;
mod gql_schema;
mod platform_executor;
mod rabbit;
mod testing;
mod utils;

use log::{error, info, warn};
use platform_executor::{
    orchestration_executor::OrchestrationExecutor, worker_executor::WorkerExecutor,
    ExecutionFaliure, Executor, GenericNode, NodeMode,
};

async fn setup_system(
    node: &mut GenericNode,
    orchestrator: &mut OrchestrationExecutor,
    worker: &mut WorkerExecutor,
) {
    match orchestrator.setup(node).await {
        Ok(_) => {
            worker.setup(node).await.unwrap();
        }
        Err(e) => {
            error!("{:?}", e);
            panic!(
                "Failed to create orchestrator, node cannot attach to or form platform. Exiting."
            )
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let orchestrator_ip = match &std::env::var("SHOULD_SCAN_NETWORK")
        .unwrap_or_else(|_| "YES".into())[..]
    {
        "NO" => {
            warn!("ENV is configured to skip network scan, this may not be desired behaviour");
            None
        }
        _ => kraken_utils::network::find_orchestrator_on_lan(crate::utils::ROCKET_PORT_NO).await,
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
        &orchestrator_ip
            .clone()
            .unwrap_or_else(|| String::from("localhost"))
    );

    info!("rabbit addr will be {}", rabbit_addr);

    let system_uuid = utils::get_system_id();

    let mut node = GenericNode::new(
        &system_uuid,
        &rabbit_addr,
        &format!(
            "{}:{}",
            &orchestrator_ip
                .clone()
                .unwrap_or_else(|| String::from("localhost")),
            &utils::ROCKET_PORT_NO
        ),
    );
    info!("node {} established", system_uuid);

    let rollover_priority = match node_mode {
        NodeMode::ORCHESTRATOR => Some(0),
        NodeMode::WORKER => {
            platform_executor::orchestration_executor::get_rollover_priority(
                &orchestrator_ip.unwrap_or_else(|| String::from("localhost")),
                &node.system_id,
            )
            .await
        }
    };

    let mut orchestrator =
        platform_executor::orchestration_executor::OrchestrationExecutor::new(rollover_priority);
    let mut worker = platform_executor::worker_executor::WorkerExecutor::new();
    setup_system(&mut node, &mut orchestrator, &mut worker).await;

    // Allows easy injection of a baseline configuration for the orchestrator
    testing::setup_experiment(&mut node, &mut orchestrator).await;

    loop {
        match orchestrator.execute(&mut node).await {
            Ok(_) => {}
            Err(faliure) => match faliure {
                ExecutionFaliure::SigKill => {
                    panic!("Orchestrator indicated a critical execution faliure, exiting")
                }
                ExecutionFaliure::BadConsumer => panic!("Worker could not connect to rabbit"),
                ExecutionFaliure::NoOrchestrator => {
                    // Orchestrator could not be found
                    // If we are here, we are beyond trying to re-connect
                    warn!("Orchestrator lost");
                    // If we lose the orchestrator, get rid of existing delployments
                    crate::platform_executor::worker_executor::clear_deployments(&mut node).await;
                    if Some(1) == orchestrator.rollover_priority {
                        // We are the primary backup, so establish ourselves as the orchestrator
                        orchestrator.rollover_priority = Some(0);
                        node.rabbit_addr = String::from("amqp://localhost:5672");
                        node.orchestrator_addr = format!("localhost:{}", &utils::ROCKET_PORT_NO);

                        // re-setup the systems
                        setup_system(&mut node, &mut orchestrator, &mut worker).await;
                    } else {
                        // We are not the primary orchestrator, so just chill and wait to find an orchestrator again
                        let backoff = vec![0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
                        for time in &backoff {
                            std::thread::sleep(std::time::Duration::from_millis(time * 1000));
                            match kraken_utils::network::find_orchestrator_on_lan(
                                crate::utils::ROCKET_PORT_NO,
                            )
                            .await
                            {
                                None => {
                                    if *time == *(backoff.last().clone().unwrap_or(&55)) {
                                        // We have failed to find an orchestrator
                                        error!("Failed to find a new orchestrator");
                                        panic!("No orchestrator, exiting");
                                    }
                                }
                                Some(_) => {
                                    orchestrator.rollover_priority = Some(0);
                                    node.rabbit_addr = String::from("amqp://localhost:5672");
                                    node.orchestrator_addr =
                                        format!("localhost:{}", &utils::ROCKET_PORT_NO);

                                    // re-setup the systems
                                    setup_system(&mut node, &mut orchestrator, &mut worker).await;
                                }
                            }
                        }
                    }
                }
            },
        };
        // An Orchestrator IS a worker, so do worker tasks too
        // This may cause problems later due to sharing of Node information? Not sure
        match worker.execute(&mut node).await {
            Ok(_) => {}
            Err(faliure) => match faliure {
                ExecutionFaliure::SigKill => {
                    panic!("Worker indicated a critical execution faliure")
                }
                ExecutionFaliure::BadConsumer => panic!("Worker could not connect to rabbit"),
                ExecutionFaliure::NoOrchestrator => panic!(
                    "Worker failed execute due to lack of orchestrator, this should never occur"
                ),
            },
        };
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[allow(unreachable_code)]
    Ok(())
}
