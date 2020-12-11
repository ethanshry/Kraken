//! The core logic for different NodeModes
pub mod orchestrator;
pub mod worker;

use crate::rabbit::RabbitBroker;
use async_trait::async_trait;
use std::time::{Duration, SystemTime};

/// The Type of the Kraken node, defines which functions must be created.
/// A Kraken device might have multiple of these roles
#[derive(Debug, Clone, PartialEq)]
pub enum NodeMode {
    /// A node only responsible for handling deployments
    WORKER,
    /// A node which both coordinates the platform and handles deployments
    ORCHESTRATOR,
}

// Defines reasons for faliure of the setup task
#[derive(Debug)]
pub enum SetupFaliure {
    NoPlatform, // Could not connect to existing platform
    BadRabbit,  // Could not create RammitMQ instance
    NoRabbit,   // Could not connect to Rabbit Instance
}

// Defines reasons for faliure of the execute task
pub enum ExecuteFaliure {
    SigKill, // Task should be killed after this execution
}

// Labeled handle to a tokio thread
pub struct Task {
    task: tokio::task::JoinHandle<()>,
    label: String,
}

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

pub struct GenericNode {
    pub deployment_processes: Vec<Task>,
    pub queue_consumers: Vec<Task>,
    pub worker_tasks: Vec<Task>,
    pub broker: Option<RabbitBroker>,
    pub system_id: String,   // TODO common
    pub rabbit_addr: String, // TODO common
    pub deployments: std::collections::LinkedList<DeploymentInfo>,
}

impl GenericNode {
    pub fn new(system_id: &str, rabbit_addr: &str) -> GenericNode {
        GenericNode {
            deployment_processes: vec![],
            queue_consumers: vec![],
            worker_tasks: vec![],
            broker: None,
            rabbit_addr: rabbit_addr.to_owned(),
            system_id: system_id.to_owned(),
            deployments: std::collections::LinkedList::new(),
        }
    }
}

/// Connects to the local RabbitMQ service
/// TODO make this more complex (i.e. exponential backoff or smtn)
pub async fn connect_to_rabbit_instance(addr: &str) -> Result<RabbitBroker, String> {
    match RabbitBroker::new(addr).await {
        Some(b) => Ok(b),
        None => Err(String::from("Failed to connect to broker")),
    }
}

// Defines the interface all nodes must satisfy
#[async_trait]
trait ExecutionNode {
    /// Is called once to set up this node
    async fn setup(&self) -> Result<(), SetupFaliure>;
    /// Is called repeatedly after setup has terminated
    async fn execute(&self) -> Result<(), ExecuteFaliure>;
}
