use crate::file_utils::clear_tmp;
use crate::platform_executor::{GenericNode, SetupFaliure, TaskFaliure};

use log::{info, warn};

pub struct Worker {}

impl Worker {
    /// Creates a new Orchestrator Object
    pub fn new() -> Worker {
        Worker {}
    }
}

pub async fn setup(node: &mut GenericNode, o: &mut Worker) -> Result<(), SetupFaliure> {
    // clear all tmp files
    clear_tmp();

    // TODO distinguish between no rabbit and no platform

    match crate::platform_executor::connect_to_rabbit_instance(&node.rabbit_addr).await {
        Ok(b) => node.broker = Some(b),
        Err(e) => {
            warn!("{}", e);
            return Err(SetupFaliure::NoRabbit);
        }
    }

    Ok(())
}

pub async fn execute(o: &Worker) -> Result<(), TaskFaliure> {
    // TODO read from work queue? Maybe a worker has a queue of deployments they should handle?
    Ok(())
}
