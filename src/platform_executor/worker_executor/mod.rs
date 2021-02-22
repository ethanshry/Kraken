//! Defines the Worker role, which handles core fucntionality of all devices on the platform
use super::{DeploymentInfo, ExecutionFaliure, Executor, GenericNode, SetupFaliure, Task};
use crate::docker::DockerBroker;
use crate::gql_model::ApplicationStatus;
use crate::rabbit::sysinfo_message::SysinfoMessage;
use crate::rabbit::{
    deployment_message::DeploymentMessage, log_message::LogMessage, QueueLabel, RabbitBroker,
    RabbitMessage,
};
use kraken_utils::file::{copy_dockerfile_to_dir, get_all_files_in_folder};
use kraken_utils::git::clone_remote_branch;
use log::{error, info};
use sysinfo::SystemExt;

mod executor;

/// This role is common to all Kraken devices.
/// Handles standard tasks- the ability to deploy applications, the monitoring of system statistics, etc
pub struct WorkerExecutor {
    /// Each WorkRequestMessage is a request of the worker by the orchestrator
    //work_requests: Arc<Mutex<Queue<WorkRequestMessage>>>,
    c: Option<lapin::Consumer>,
}

impl WorkerExecutor {
    /// Creates a new Orchestrator Object
    pub fn new() -> WorkerExecutor {
        WorkerExecutor {
            c: None, //work_requests: Arc::new(Mutex::new(Queue::new())),
        }
    }
}

impl WorkerExecutor {
    /// Returns a task which publishes system info to RabbitMQ
    pub async fn get_publish_node_system_stats_task(
        node: &GenericNode,
    ) -> tokio::task::JoinHandle<()> {
        let system_status_proc = {
            let system_uuid = node.system_id.clone();

            let broker = match Self::connect_to_rabbit_instance(&node.rabbit_addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };

            let publisher = broker.get_channel().await;

            tokio::spawn(async move {
                let lan_addr = kraken_utils::network::get_lan_addr();
                let mut msg = SysinfoMessage::new(
                    &system_uuid,
                    &kraken_utils::cli::get_node_name(),
                    &lan_addr.unwrap_or_else(|| String::from("127.0.0.1")),
                );
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
}

/// Deploys an application instance via docker
pub async fn handle_deployment(
    system_id: &str,
    broker: &RabbitBroker,
    git_uri: &str,
    git_branch: &str,
    id: &str,
) -> Result<DeploymentInfo, DeploymentInfo> {
    let container_guid = String::from(id);

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    // TODO make function to execute a thing in a tmp dir which auto-cleans itself (#51)
    std::fs::remove_dir_all(&format!("tmp/deployment/{}", id));
    let tmp_dir_path = &format!("tmp/deployment/{}", id);

    info!("Creating Container {}", &container_guid);

    msg.update_message(
        ApplicationStatus::RetrievingApplicationData,
        &format!("Retrieving Application Data from {}", git_uri),
        None,
    );

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    info!("Retrieving git repository for container from {}", git_uri);
    clone_remote_branch(git_uri, git_branch, tmp_dir_path).unwrap();

    let mut deployment_info = DeploymentInfo::new(&container_guid, "", false);

    let deployment_config = match crate::deployment::shipwreck::get_config_for_path(&format!(
        "{}/shipwreck.toml",
        &tmp_dir_path
    )) {
        Some(c) => c,
        None => {
            msg.update_message(
                ApplicationStatus::Errored,
                &"shipwreck.toml not detected or incompatible format, please ensure your file is compliant",
                None
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            return Err(deployment_info);
        }
    };

    let port = deployment_config.config.port;

    deployment_info.update_address(&format!("{}", port));

    // Create deployment info before first docker command, so build logs will be preserved

    let dockerfile_name = match &deployment_config.config.lang[..] {
        "python3" => Some("python36.dockerfile"),
        "node" => Some("node.dockerfile"),
        "custom" => None,
        _ => {
            msg.update_message(
                ApplicationStatus::Errored,
                &"shipwreck.toml specified an unsupported language. check the Kraken-UI for supported languages",
                None
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            return Err(deployment_info);
        }
    };

    match dockerfile_name {
        Some(file) => {
            copy_dockerfile_to_dir(file, tmp_dir_path);
        }
        None => {
            // We are using a custom dockerfile, make sure it exists
            let files = get_all_files_in_folder(tmp_dir_path).unwrap();
            if !files.contains(&format!("{}/Dockerfile", tmp_dir_path)) {
                msg.update_message(
                    ApplicationStatus::Errored,
                    &"The Deployment is configured to use a custom Dockerfile, and one could not be found in the repository root.",
                    None
                );
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                return Err(deployment_info);
            }
        }
    }

    info!("module path is {}", module_path!());

    msg.update_message(ApplicationStatus::BuildingDeployment, "", None);

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    println!("CANNOT SEND RIP US");

    let docker = DockerBroker::new().await;

    println!("CANNOT DOCK RIP US");
    if let Some(docker) = docker {
        let res = docker
            .build_image(tmp_dir_path, Some(container_guid.to_string()))
            .await;
        if let Ok(r) = res {
            info!("----- Docker Build Results for {} -----", r.image_id);
            info!("{:?}", r.log);
            msg.update_message(
                ApplicationStatus::BuildingDeployment,
                &r.log.join("\n"),
                None,
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            msg.update_message(
                ApplicationStatus::BuildingDeployment,
                &format!(
                    "docker image created for {} with id {}",
                    &git_uri,
                    &r.log.join("\n")
                ),
                None,
            );

            // Send build logs to appropriate logfile
            let mut log_msg = LogMessage::new(&container_guid);
            log_msg.update_message(&r.log.join("\n"));
            log_msg.send(&publisher, QueueLabel::Log.as_str()).await;

            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            // Image build went ok, now deploy the container
            msg.update_message(ApplicationStatus::Deploying, "", None);
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            let ids = docker.start_container(&r.image_id, port).await;

            if let Ok(id) = ids {
                info!("Docker container started for {} with id {}", git_uri, id);
                let status = docker.get_container_status(&id).await;
                msg.update_message(ApplicationStatus::Running, &id, status);
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            } else {
                msg.update_message(ApplicationStatus::Errored, "Error in deployment", None);
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                return Err(deployment_info);
            }
        } else {
            msg.update_message(ApplicationStatus::Errored, "Error in build process", None);
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            error!("Failed to build docker image for {}", &git_uri);
            return Err(deployment_info);
        }
    }
    deployment_info.deployment_is_ok(true);
    Ok(deployment_info)
}

/// Terminates a docker deployment
pub async fn kill_deployment(
    system_id: &str,
    broker: &RabbitBroker,
    container_id: &str,
) -> Result<String, ()> {
    let container_guid = String::from(container_id);

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);
    msg.update_message(ApplicationStatus::DestructionInProgress, "", None);
    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    let docker = DockerBroker::new().await;
    if let Some(docker) = docker {
        docker.stop_container(&container_guid).await;
        docker.prune_images(Some("1s")).await;
        docker.prune_containers(Some("1s")).await;
        msg.update_message(ApplicationStatus::Destroyed, "", None);
        msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
        return Ok(container_guid.to_string());
    }
    Err(())
}
