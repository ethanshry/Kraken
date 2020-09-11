use crate::file_utils::clear_tmp;
use crate::platform_executor::{GenericNode, SetupFaliure, Task, TaskFaliure};
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    sysinfo_message::SysinfoMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use log::{info, warn};
use queues::{IsQueue, Queue};
use std::sync::{Arc, Mutex};

pub struct Worker {
    work_requests: Arc<Mutex<Queue<WorkRequestMessage>>>,
}

impl Worker {
    /// Creates a new Orchestrator Object
    pub fn new() -> Worker {
        Worker {
            work_requests: Arc::new(Mutex::new(Queue::new())),
        }
    }
}

pub async fn setup(node: &mut GenericNode, w: &mut Worker) -> Result<(), SetupFaliure> {
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

    // Declare rabbit queue
    node.broker
        .as_ref()
        .unwrap()
        .declare_queue(&node.system_id)
        .await;

    // send system stats
    let publish_node_stats_task =
        crate::platform_executor::get_publish_node_system_stats_task(node).await;
    node.worker_tasks.push(Task {
        task: publish_node_stats_task,
        label: String::from("NodeStats"),
    });

    // consumer personal work queue
    // TODO be better at this clone move stuff
    let sysinfo_consumer = {
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let arc = w.work_requests.clone();

        let handler = move |data: Vec<u8>| {
            let mut work_queue = arc.lock().unwrap();
            let (node, message) = WorkRequestMessage::deconstruct_message(&data);
            info!("Recieved Message on {}'s Work Queue", &system_uuid);
            work_queue.add(message).unwrap();
            return ();
        };
        let system_uuid = node.system_id.clone();
        tokio::spawn(async move {
            let broker = match crate::platform_executor::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, &system_uuid, &handler)
                .await;
        })
    };
    node.queue_consumers.push(Task {
        task: sysinfo_consumer,
        label: String::from(&node.system_id),
    });

    Ok(())
}

pub async fn execute(node: &GenericNode, w: &mut Worker) -> Result<(), TaskFaliure> {
    // Each execution will perform a single task in the work queue.
    // If more work needs to be completed, execute should be called again
    let arc = w.work_requests.clone();
    let mut work_queue = arc.lock().unwrap();
    if let Ok(_) = work_queue.peek() {
        let task = work_queue.remove().unwrap();
        info!("{:?}", task);
    }
    drop(work_queue);
    Ok(())
}
