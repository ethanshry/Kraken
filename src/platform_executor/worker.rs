use crate::db::{Database, ManagedDatabase};
use crate::file_utils::{clear_tmp, copy_dir_contents_to_static};
use crate::git_utils::clone_remote_branch;
use crate::model::{Platform, Service, ServiceStatus};
use crate::platform_executor::{ExecutionNode, GenericNode, SetupFaliure, Task, TaskFaliure};
use crate::rabbit::{
    deployment_message::DeploymentMessage, sysinfo_message::SysinfoMessage, QueueLabel,
    RabbitBroker, RabbitMessage,
};
use crate::schema::Query;
use async_trait::async_trait;
use dotenv;
use juniper::EmptyMutation;
use log::{info, warn};
use std::fs;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;

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
