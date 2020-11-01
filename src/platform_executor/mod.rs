pub mod orchestrator;
pub mod worker;

use crate::rabbit::{sysinfo_message::SysinfoMessage, QueueLabel, RabbitBroker, RabbitMessage};
use async_trait::async_trait;
use std::time::{Duration, SystemTime};
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
