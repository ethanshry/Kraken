//! Defines the Worker role, which handles core fucntionality of all devices on the platform
use super::{
    handle_deployment, kill_deployment, ExecutionFaliure, Executor, GenericNode, SetupFaliure,
    Task, WorkerExecutor,
};
use crate::docker::DockerBroker;
use crate::file_utils::clear_tmp;
use crate::rabbit::{
    log_message::LogMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use async_trait::async_trait;
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
