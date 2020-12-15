//! Defines the Worker role, which handles core fucntionality of all devices on the platform
use super::{DeploymentInfo, ExecutionFaliure, Executor, GenericNode, SetupFaliure, Task};
use crate::docker::DockerBroker;
use crate::file_utils::{clear_tmp, copy_dockerfile_to_dir};
use crate::git_utils::clone_remote_branch;
use crate::gql_model::ApplicationStatus;
use crate::rabbit::sysinfo_message::SysinfoMessage;
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    log_message::LogMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitBroker, RabbitMessage,
};
use async_trait::async_trait;
use log::{error, info, warn};
use sysinfo::SystemExt;

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

#[async_trait]
impl Executor for WorkerExecutor {
    /// The tasks associated with setting up this role.
    /// Workers are primarilly concerned with connecting to RabbitMQ, and establishing necesarry queues
    async fn setup(&mut self, node: &mut GenericNode) -> Result<(), SetupFaliure> {
        clear_tmp();

        match Self::connect_to_rabbit_instance(&node.rabbit_addr).await {
            Ok(b) => node.broker = Some(b),
            Err(e) => {
                warn!("{}", e);
                return Err(SetupFaliure::NoRabbit);
            }
        }

        info!("Declared rabbit queue for {}", &node.system_id);
        node.broker
            .as_ref()
            .unwrap()
            .declare_queue(&node.system_id)
            .await;

        // send system stats
        let publish_node_stats_task =
            WorkerExecutor::get_publish_node_system_stats_task(node).await;
        node.worker_tasks.push(Task {
            task: publish_node_stats_task,
            label: String::from("NodeStats"),
        });

        // consumer personal work queue
        // TODO be better at this clone move stuff

        let broker = match Self::connect_to_rabbit_instance(&node.rabbit_addr).await {
            Ok(b) => b,
            Err(_) => panic!("Could not establish rabbit connection"),
        };
        self.c = Some(
            broker
                .consume_queue_incr(&node.system_id, &node.system_id)
                .await,
        );

        Ok(())
    }

    /// Logic which should be executed every iteration
    /// Primarilly focused on handling deployment/kill/update requests, and processing logs
    async fn execute(&mut self, node: &mut GenericNode) -> Result<(), ExecutionFaliure> {
        // Each execution will perform a single task in the work queue.
        // If more work needs to be completed, execute should be called again

        match &mut self.c {
            Some(c) => {
                let item = crate::rabbit::util::try_fetch_consumer_item(c).await;
                if let Some(data) = item {
                    let (_, task) = WorkRequestMessage::deconstruct_message(&data);
                    info!("{:?}", task);

                    match task.request_type {
                        WorkRequestType::RequestDeployment => {
                            let res = handle_deployment(
                                &node.system_id,
                                node.broker.as_ref().unwrap(),
                                &task.deployment_url.unwrap(),
                                &task.deployment_id.unwrap(),
                            )
                            .await;
                            if let Ok(r) = res {
                                node.deployments.push_back(r);
                            }
                        }
                        WorkRequestType::CancelDeployment => {
                            let res = kill_deployment(
                                &node.system_id,
                                node.broker.as_ref().unwrap(),
                                &task.deployment_id.unwrap(),
                            )
                            .await;
                            if let Ok(r) = res {
                                for d in node.deployments.iter_mut() {
                                    if d.deployment_id == r {
                                        d.deployment_is_ok(false);
                                        break;
                                    }
                                }
                            }
                        }
                        _ => info!("Request type not handled"),
                    }
                }
            }
            None => return Err(ExecutionFaliure::BadConsumer),
        }

        let mut nodes_to_remove = vec![];
        for (index, d) in node.deployments.iter_mut().enumerate() {
            // if more than a second has passed, check for logs
            if d.last_log_time
                .elapsed()
                .unwrap_or_else(|_| std::time::Duration::new(0, 0))
                .as_secs()
                > 1
            {
                let docker = DockerBroker::new().await;

                if let Some(docker) = docker {
                    let logs = docker.get_logs(&d.deployment_id, d.last_log_time).await;
                    d.update_log_time();
                    if !logs.is_empty() {
                        let publisher = node.broker.as_ref().unwrap().get_channel().await;
                        let mut msg = LogMessage::new(&d.deployment_id);
                        msg.update_message(&logs.join(""));
                        msg.send(&publisher, QueueLabel::Log.as_str()).await;
                    }
                }
            }

            if d.status.is_err() {
                nodes_to_remove.push(index);
            }
        }

        nodes_to_remove.reverse();

        for index in nodes_to_remove.iter() {
            let mut split_list = node.deployments.split_off(*index);
            split_list.pop_front();
            node.deployments.append(&mut split_list);
        }
        Ok(())
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
}

/// Deploys an application instance via docker
pub async fn handle_deployment(
    system_id: &str,
    broker: &RabbitBroker,
    git_uri: &str,
    id: &str,
) -> Result<DeploymentInfo, DeploymentInfo> {
    let container_guid = String::from(id);

    let publisher = broker.get_channel().await;

    let mut msg = DeploymentMessage::new(&system_id, &container_guid);

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    // TODO make function to execute a thing in a tmp dir which auto-cleans itself (#51)
    let tmp_dir_path = &format!("tmp/deployment/{}", id);

    info!("Creating Container {}", &container_guid);

    msg.update_message(
        ApplicationStatus::RetrievingApplicationData,
        &format!("Retrieving Application Data from {}", git_uri),
    );

    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    info!("Retrieving git repository for container from {}", git_uri);
    clone_remote_branch(git_uri, "main", tmp_dir_path)
        .wait()
        .unwrap();

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
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            return Err(deployment_info);
        }
    };

    let port = deployment_config.config.port;

    deployment_info.update_address(&format!("{}", port));

    // Create deployment info before first docker command, so build logs will be preserved

    let dockerfile_name = match &deployment_config.config.lang[..] {
        "python3" => "python36.dockerfile",
        "node" => "node.dockerfile",
        _ => {
            msg.update_message(
                ApplicationStatus::Errored,
                &"shipwreck.toml specified an unsupported language. check the Kraken-UI for supported languages",
            );
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            return Err(deployment_info);
        }
    };

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

            // Send build logs to appropriate logfile
            let mut log_msg = LogMessage::new(&container_guid);
            log_msg.update_message(&r.log.join("\n"));
            log_msg.send(&publisher, QueueLabel::Log.as_str()).await;

            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            // Image build went ok, now deploy the container
            msg.update_message(ApplicationStatus::Deploying, "");
            msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

            let ids = docker.start_container(&r.image_id, port).await;

            if let Ok(id) = ids {
                info!("Docker container started for {} with id {}", git_uri, id);
                msg.update_message(ApplicationStatus::Running, &id);
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
            } else {
                msg.update_message(ApplicationStatus::Errored, "Error in deployment");
                msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                return Err(deployment_info);
            }
        } else {
            msg.update_message(ApplicationStatus::Errored, "Error in build process");
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
    msg.update_message(ApplicationStatus::DestructionInProgress, "");
    msg.send(&publisher, QueueLabel::Deployment.as_str()).await;

    let docker = DockerBroker::new().await;
    if let Some(docker) = docker {
        docker.stop_container(&container_guid).await;
        docker.prune_images(Some("1s")).await;
        docker.prune_containers(Some("1s")).await;
        msg.update_message(ApplicationStatus::Destroyed, "");
        msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
        return Ok(container_guid.to_string());
    }
    Err(())
}
