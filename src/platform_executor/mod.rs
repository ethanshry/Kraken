//! The core logic for the platform
//! This module encompases two primary executors
//! * `OrchestrationExecutors` - Handles logic in reguards to platform creation, rollover, and deployment coordination
//! * `WorkerExecutor` - Handles logic in relation to the management and monitoring of Docker Containers for Deployments

pub mod orchestration_executor;
pub mod worker_executor;

use crate::rabbit::RabbitBroker;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// The Type of the Kraken node, defines which functions must be created.
/// A Kraken device might have multiple of these roles
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeMode {
    /// A node only responsible for handling deployments
    WORKER,
    /// A node which both coordinates the platform and handles deployments
    ORCHESTRATOR,
}

/// Defines reasons for failure of the setup task
#[derive(Debug)]
pub enum SetupFailure {
    /// Could not connect to existing platform
    NoPlatform,
    /// Could not create RammitMQ instance
    BadRabbit,
    /// Could not connect to Rabbit Instance
    NoRabbit,
}

/// Defines reasons for failure of the execute task
pub enum ExecutionFailure {
    /// Task should be killed after this execution
    SigKill,
    /// Error in accessing consumer or parsing message
    BadConsumer,
    /// Error in communicating with orchestrator
    NoOrchestrator,
}

/// Labeled handle to a tokio thread
pub struct Task {
    task: tokio::task::JoinHandle<()>,
    label: String,
}

/// Information about a Deployment
pub struct DeploymentInfo {
    pub deployment_id: String,
    pub address: String,
    pub last_log_time: SystemTime,
    pub status: Result<(), ()>,
}

impl DeploymentInfo {
    pub fn new(deployment_id: &str, address: &str, is_ok: bool) -> DeploymentInfo {
        DeploymentInfo {
            deployment_id: deployment_id.to_string(),
            address: address.to_string(),
            last_log_time: SystemTime::now(),
            status: if is_ok { Ok(()) } else { Err(()) },
        }
    }

    pub fn deployment_is_ok(&mut self, is_ok: bool) {
        self.status = if is_ok { Ok(()) } else { Err(()) };
    }

    pub fn update_address(&mut self, address: &str) {
        self.address = address.to_string();
    }

    pub fn update_log_time(&mut self) {
        self.last_log_time = SystemTime::now().checked_add(Duration::new(1, 0)).unwrap();
    }
}

/// Data struct which contains data that should be shared between Executors
/// Or data which we don't want to lose access to if one executor crashes
pub struct GenericNode {
    pub broker: Option<RabbitBroker>,
    pub system_id: String,
    pub rabbit_addr: String,
    pub orchestrator_addr: String,
}

impl GenericNode {
    pub fn new(system_id: &str, rabbit_addr: &str, orchestrator_addr: &str) -> GenericNode {
        GenericNode {
            broker: None,
            rabbit_addr: rabbit_addr.to_owned(),
            orchestrator_addr: orchestrator_addr.to_owned(),
            system_id: system_id.to_owned(),
        }
    }
}

/// An executor is a way to group collections of functionality under different roles
/// Each implementor should expect setup to be called once, to set up, and then for execute to be called repeatedly so long as the executor is alive.
#[async_trait]
pub trait Executor {
    /// Is called once to set up this node
    async fn setup(&mut self, node: &mut GenericNode) -> Result<(), SetupFailure>;
    /// Is called repeatedly after setup has terminated
    async fn execute(&mut self, node: &mut GenericNode) -> Result<(), ExecutionFailure>;
    /// Connects to the local RabbitMQ service
    /// TODO make this smarter (i.e. exponential backoff or smtn)
    async fn connect_to_rabbit_instance(addr: &str) -> Result<RabbitBroker, String> {
        match RabbitBroker::new(addr).await {
            Some(b) => Ok(b),
            None => Err(String::from("Failed to connect to broker")),
        }
    }
}
