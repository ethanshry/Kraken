pub mod orchestrator;
pub mod worker;

use crate::rabbit::{
    deployment_message::DeploymentMessage, sysinfo_message::SysinfoMessage, QueueLabel,
    RabbitBroker, RabbitMessage,
};
use async_trait::async_trait;
use sysinfo::SystemExt;

#[derive(Debug)]
pub enum SetupFaliure {
    NoPlatform, // Could not connect to existing platform
    BadRabbit,  // Could not create RammitMQ instance
    NoRabbit,   // Could not connect to Rabbit Instance
}

pub enum TaskFaliure {
    SigKill, // Task should be killed after this execution
}

pub struct Task {
    task: tokio::task::JoinHandle<()>,
    label: String,
}

pub struct GenericNode {
    pub deployment_processes: Vec<Task>,
    pub queue_consumers: Vec<Task>,
    pub worker_tasks: Vec<Task>,
    pub broker: Option<RabbitBroker>,
    pub system_id: String,   // TODO common
    pub rabbit_addr: String, // TODO common
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
        }
    }
}

// TODO make this more complex (i.e. exponential backoff or smtn)
pub async fn connect_to_rabbit_instance(addr: &str) -> Result<RabbitBroker, String> {
    match RabbitBroker::new(addr).await {
        Some(b) => Ok(b),
        None => Err(String::from("Failed to connect to broker")),
    }
}

pub async fn get_publish_node_system_stats_task(node: &GenericNode) -> tokio::task::JoinHandle<()> {
    let system_status_proc = {
        let system_uuid = node.system_id.clone();

        let broker =
            match crate::platform_executor::connect_to_rabbit_instance(&node.rabbit_addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };

        let publisher = broker.get_channel().await;

        tokio::spawn(async move {
            let mut msg = SysinfoMessage::new(&system_uuid);
            loop {
                std::thread::sleep(std::time::Duration::new(5, 0));
                let system = sysinfo::System::new_all();
                msg.update_message(
                    system.get_free_memory(),
                    system.get_used_memory(),
                    system.get_uptime(),
                    system.get_load_average().five as f32,
                );
                msg.send(&publisher, QueueLabel::Sysinfo.as_str()).await;
            }
        })
    };
    system_status_proc
}

#[async_trait]
trait ExecutionNode {
    async fn setup(&self) -> Result<(), SetupFaliure>;
    async fn execute(&self) -> Result<(), TaskFaliure>;
}
