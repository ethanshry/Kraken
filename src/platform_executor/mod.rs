pub mod orchestrator;

use crate::rabbit::RabbitBroker;
use async_trait::async_trait;

pub enum SetupFaliure {
    NoPlatform, // Could not connect to existing platform
    NoRabbit,   // Could not connect to Rabbit Instance
}

pub enum TaskFaliure {
    SigKill, // Task should be killed after this execution
}

pub struct DeploymentTask {
    task: tokio::task::JoinHandle<()>,
    id: String,
}

pub struct QueueTask {
    task: tokio::task::JoinHandle<()>,
    queue_label: String,
}

pub struct GenericNode {
    pub deployment_processes: Option<Vec<DeploymentTask>>,
    pub queue_consumers: Option<Vec<QueueTask>>,
    pub broker: Option<RabbitBroker>,
    pub system_id: String,   // TODO common
    pub rabbit_addr: String, // TODO common
}

impl GenericNode {
    pub fn new(system_id: &str, rabbit_addr: &str) -> GenericNode {
        GenericNode {
            deployment_processes: None,
            queue_consumers: None,
            broker: None,
            rabbit_addr: rabbit_addr.to_owned(),
            system_id: system_id.to_owned(),
        }
    }
}

#[async_trait]
trait NodeUtils {
    async fn connect_to_rabbit_instance(&self) -> Result<RabbitBroker, String>;
}

#[async_trait]
impl NodeUtils for GenericNode {
    async fn connect_to_rabbit_instance(&self) -> Result<RabbitBroker, String> {
        match RabbitBroker::new(&self.rabbit_addr).await {
            Some(b) => Ok(b),
            None => Err(String::from("Failed to connect to broker")),
        }
    }
}

#[async_trait]
trait ExecutionNode {
    async fn setup(&self) -> Result<(), SetupFaliure>;
    async fn execute(&self) -> Result<(), TaskFaliure>;
}
