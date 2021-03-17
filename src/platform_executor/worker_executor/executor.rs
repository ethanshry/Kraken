//! Defines the Worker role, which handles core fucntionality of all devices on the platform
use super::{
    handle_deployment, kill_deployment, ExecutionFaliure, Executor, GenericNode, SetupFaliure,
    Task, WorkerExecutor,
};
use crate::docker::DockerBroker;
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    log_message::LogMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use async_trait::async_trait;
use kraken_utils::file::clear_tmp;
use log::{info, warn};

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
        self.tasks.push(Task {
            task: publish_node_stats_task,
            label: String::from("NodeStats"),
        });

        // consumer personal work queue
        // TODO be better at this clone move stuff

        let broker = match Self::connect_to_rabbit_instance(&node.rabbit_addr).await {
            Ok(b) => b,
            Err(_) => panic!("Could not establish rabbit connection"),
        };
        self.work_queue_consumer = Some(
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
        // If more work needs to be completed, it will happen when execute is next called

        match &mut self.work_queue_consumer {
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
                                &task.git_branch.unwrap(),
                                &task.deployment_id.unwrap(),
                            )
                            .await;
                            if let Ok(r) = res {
                                self.deployments.push_back(r);
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
                                for d in self.deployments.iter_mut() {
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

        let mut deployments_to_remove = vec![];
        for (index, d) in self.deployments.iter_mut().enumerate() {
            // if more than a second has passed, check for logs and get updated deployment status
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

                    if let Some(status) = docker.get_container_status(&d.deployment_id).await {
                        let state = match status.state {
                            bollard::models::ContainerStateStatusEnum::RUNNING => {
                                crate::gql_model::ApplicationStatus::Running
                            }
                            _ => crate::gql_model::ApplicationStatus::Errored,
                        };
                        let description = match state {
                            crate::gql_model::ApplicationStatus::Running => {
                                String::from("Application status normal")
                            }
                            _ => format!(
                                "Docker reported error in application, state {:?}",
                                status.state
                            ),
                        };
                        let publisher = node.broker.as_ref().unwrap().get_channel().await;
                        let mut msg = DeploymentMessage::new(&node.system_id, &d.deployment_id);
                        msg.update_message(state, &description, Some(status));
                        msg.send(&publisher, QueueLabel::Deployment.as_str()).await;
                    }
                }
            }

            if d.status.is_err() {
                deployments_to_remove.push(index);
            }
        }

        deployments_to_remove.reverse();

        for index in deployments_to_remove.iter() {
            let mut split_list = self.deployments.split_off(*index);
            split_list.pop_front();
            self.deployments.append(&mut split_list);
        }
        Ok(())
    }
}
