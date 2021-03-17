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

/// Sets up the orhcestration and worker executors for a system
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

/// Primary Application Entrypoint
#[tokio::main]
async fn main() -> Result<(), ()> {
    // Establish logging
    dotenv::dotenv().ok();
    env_logger::init();

    // Try to find a platform to attach to
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

    // Either we are attaching to a rabbit instance, or we will be the rabbit instance
    let rabbit_addr: String = format!(
        "amqp://{}:5672",
        &orchestrator_ip
            .clone()
            .unwrap_or_else(|| String::from("localhost"))
    );

    info!("rabbit addr will be {}", rabbit_addr);

    let system_uuid = utils::get_system_id();

    // Create shared data storage for our executors
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

    // Figure out our rollover priority based on whether we are attaching to a platform or creating one
    // This priority is used by the Orchestration Executor to figure out what its responsibilities should be
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

    // We have all the information we need to establish our executors, so establish them
    let mut orchestrator =
        platform_executor::orchestration_executor::OrchestrationExecutor::new(rollover_priority);
    let mut worker = platform_executor::worker_executor::WorkerExecutor::new();
    setup_system(&mut node, &mut orchestrator, &mut worker).await;

    // Allows easy injection of a baseline configuration for the orchestrator
    // Currently does nothing
    testing::setup_experiment(&mut node, &mut orchestrator).await;

    // The main execution loop
    loop {
        // Each node has an orchestration executor and a worker executor
        // Try to do orchestration tasks
        match orchestrator.execute(&mut node).await {
            Ok(_) => {}
            // In the case of faliures, figure out what kind of faliure it is and handle recovery appropriately
            Err(faliure) => match faliure {
                ExecutionFaliure::SigKill => {
                    panic!("Orchestrator indicated a critical execution faliure, exiting")
                }
                // If we can't connect to rabbitMQ as the orchestrator, something got very messed up with Docker.
                // Only primary orchestrators will have this error, so safest bet is to just crash and let platform recover itself
                ExecutionFaliure::BadConsumer => panic!("Worker could not connect to rabbit"),
                // Rollover Candidate cannot detect primary orchestrator, try to reconnect or roll over
                ExecutionFaliure::NoOrchestrator => {
                    // Orchestrator could not be found
                    // If we are here, we are beyond trying to re-connect
                    warn!("Orchestrator lost");
                    // If we lose the orchestrator, get rid of existing delployments
                    worker.clear_deployments(&mut node).await;
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
        match worker.execute(&mut node).await {
            Ok(_) => {}
            // In the case of faliure in worker tasks, we just want to fail. Typically errors here will be caught by the Orchestrator first
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
        // TODO delta timing would be much better here
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    #[allow(unreachable_code)]
    Ok(())
}
