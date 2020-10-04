use crate::docker::DockerBroker;
use crate::file_utils::{clear_tmp, copy_dockerfile_to_dir};
use crate::git_utils::clone_remote_branch;
use crate::model::ApplicationStatus;
use crate::platform_executor::{GenericNode, SetupFaliure, Task, TaskFaliure};
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitBroker, RabbitMessage,
};
use log::{error, info, warn};
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
            let (_node, message) = WorkRequestMessage::deconstruct_message(&data);
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
        let task: WorkRequestMessage = work_queue.remove().unwrap();
        info!("{:?}", task);

        match task.request_type {
            WorkRequestType::RequestDeployment => {
                handle_deployment(
                    &node.system_id,
                    node.broker.as_ref().unwrap(),
                    &task.deployment_url.unwrap(),
                    &task.deployment_id.unwrap(),
                )
                .await
                .unwrap();
            }
            WorkRequestType::CancelDeployment => {
                kill_deployment(
                    &node.system_id,
                    node.broker.as_ref().unwrap(),
                    &task.deployment_id.unwrap(),
                )
                .await
                .unwrap();
            }
            _ => info!("Request type not handled"),
        }
    }
    drop(work_queue);
    Ok(())
}

pub async fn handle_deployment(
    system_id: &str,
    broker: &RabbitBroker,
    git_uri: &str,
    id: &str,
) -> Result<(), ()> {
    let container_guid = id.clone();

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    // TODO make function to execute a thing in a tmp dir which auto-cleans itself
    let tmp_dir_path = &format!("tmp/deployment/{}", id);

    info!("Creating Container {}", &container_guid);

    msg.update_message(
        ApplicationStatus::RetrievingApplicationData,
        &format!("Retrieving Application Data from {}", git_uri),
    );

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    info!("Retrieving git repository for container from {}", git_uri);
    clone_remote_branch(git_uri, "master", tmp_dir_path)
        .wait()
        .unwrap();

    // Inject the proper dockerfile into the project
    // TODO read configuration information from toml file

    let dockerfile_name = "python36.dockerfile";

    copy_dockerfile_to_dir(dockerfile_name, tmp_dir_path);

    info!("module path is {}", module_path!());

    msg.update_message(ApplicationStatus::BuildingDeployment, "");

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    let docker = DockerBroker::new().await;
    if let Some(docker) = docker {
        let res = docker
            .build_image(tmp_dir_path, Some(container_guid.to_string()))
            .await;
        if let Ok(r) = res {
            info!("----- Docker Build Results for {} -----", r.image_id);
            info!("{:?}", r.log);
            msg.update_message(ApplicationStatus::BuildingDeployment, &r.log.join("\n"));
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            msg.update_message(
                ApplicationStatus::BuildingDeployment,
                &format!(
                    "docker image created for {} with id {}",
                    &git_uri,
                    &r.log.join("\n")
                ),
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            // Image build went ok, now deploy the container
            msg.update_message(ApplicationStatus::Deploying, "");
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            let ids = docker.start_container(&r.image_id, 9000).await;

            if let Ok(id) = ids {
                info!("Docker container started for {} with id {}", git_uri, id);
                msg.update_message(ApplicationStatus::Running, &format!("{}", id));
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            } else {
                msg.update_message(ApplicationStatus::Errored, "Error in deployment");
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                return Err(());
            }
        } else {
            msg.update_message(ApplicationStatus::Errored, "Error in build process");
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            error!("Failed to build docker image for {}", &git_uri);
            return Err(());
        }
    }

    Ok(())
}

pub async fn kill_deployment(system_id: &str, broker: &RabbitBroker, id: &str) -> Result<(), ()> {
    let container_guid = id.clone();

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);
    msg.update_message(ApplicationStatus::DestructionInProgress, "");
    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    let docker = DockerBroker::new().await;
    if let Some(docker) = docker {
        docker.stop_container(&container_guid).await;
        docker.prune_images(Some("1s")).await;
        msg.update_message(ApplicationStatus::Destroyed, "");
        msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
        return Ok(());
    }
    Err(())
}
