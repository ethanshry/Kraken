//! Executor impl for OrchestrationExecutor

use super::{ExecutionFaliure, Executor, GenericNode, OrchestrationExecutor, SetupFaliure, Task};
use crate::gql_model::{ApplicationStatus, Node};
use crate::network::get_lan_addr;
use crate::rabbit::{
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use crate::{cli_utils, file_utils::clear_tmp};
use async_trait::async_trait;
use futures::future;
use log::{error, info, warn};
use std::fs;

#[async_trait]
impl Executor for OrchestrationExecutor {
    /// The tasks associated with setting up this role.
    /// Workers are primarilly concerned with connecting to RabbitMQ, and establishing necesarry queues
    async fn setup(&mut self, node: &mut GenericNode) -> Result<(), SetupFaliure> {
        // Put DB interaction in block scope
        // This prevents us from needing a `Send` `MutexGuard`
        let lan_addr = get_lan_addr();
        {
            let arc = self.db_ref.clone();
            let mut db = arc.lock().unwrap();
            db.insert_node(&Node::new(
                &node.system_id,
                &cli_utils::get_node_name(),
                &lan_addr.unwrap_or_else(|| String::from("127.0.0.1")),
            ));
        }

        // prepare local directories
        clear_tmp();
        fs::create_dir_all(crate::utils::LOG_LOCATION).unwrap();

        // setup rabbitmq and UI
        let ui = OrchestrationExecutor::fetch_ui(self.db_ref.clone());
        let rabbit = OrchestrationExecutor::deploy_rabbit_instance(self.db_ref.clone(), &node);
        let tasks = future::join(ui, rabbit);
        let (_ui_res, rabbit_res) = tasks.await;

        if let Err(e) = rabbit_res {
            error!("RabbitMQ Setup Failed with error: {}", e);
            return Err(SetupFaliure::BadRabbit);
        }

        // Try to make connection
        match Self::connect_to_rabbit_instance(&node.rabbit_addr).await {
            Ok(b) => node.broker = Some(b),
            Err(e) => {
                warn!("{}", e);
                return Err(SetupFaliure::NoRabbit);
            }
        }

        info!("Succesfully established RabbitMQ service");

        if let Err(()) = self
            .establish_rabbit_queues(&node.broker.as_ref().unwrap())
            .await
        {
            return Err(SetupFaliure::BadRabbit);
        }

        // Consume RabbitMQ Queues
        let sysinfo_consumer = self.get_sysinfo_consumer(node);
        node.queue_consumers.push(Task {
            task: sysinfo_consumer,
            label: String::from(QueueLabel::Sysinfo.as_str()),
        });
        let deployment_consumer = self.get_deployment_consumer(node);
        node.queue_consumers.push(Task {
            task: deployment_consumer,
            label: String::from(QueueLabel::Deployment.as_str()),
        });
        let log_consumer = self.get_log_consumer(node);
        node.queue_consumers.push(Task {
            task: log_consumer,
            label: String::from(QueueLabel::Log.as_str()),
        });
        self.api_server = Some(OrchestrationExecutor::create_api_server(
            self.db_ref.clone(),
        ));
        Ok(())
    }

    /// Logic which should be executed every iteration
    /// Primarilly focused on handling deployment/kill/update requests, and processing logs
    async fn execute(&mut self, node: &mut GenericNode) -> Result<(), ExecutionFaliure> {
        // TODO look for work to distribute and do it

        let deployments;
        {
            // Look through deployments for new deployments which need to be scheduled
            let arc = self.db_ref.clone();
            let db = arc.lock().unwrap();
            deployments = db.get_deployments();
        }
        if let Some(d) = deployments {
            for mut deployment in d {
                match deployment.status.0 {
                    ApplicationStatus::DeploymentRequested => {
                        // Look for free nodes to distribute tasks to

                        deployment.update_status(&ApplicationStatus::ValidatingDeploymentData);
                        self.update_deployment_in_db(&deployment);
                        match super::validate_deployment(
                            &deployment.src_url,
                            &deployment.git_branch,
                        )
                        .await
                        {
                            Err(_) => {
                                warn!("Deployment failed validation {}", &deployment.id);
                                {
                                    let arc = self.db_ref.clone();
                                    let mut db = arc.lock().unwrap();
                                    deployment.update_status(&ApplicationStatus::Errored);
                                    db.update_deployment(&deployment.id, &deployment);
                                }
                            }
                            Ok((commit, shipwreck_string)) => {
                                let deployment_config =
                                    crate::deployment::shipwreck::get_config_for_string(
                                        &shipwreck_string,
                                    );

                                deployment.update_status(&ApplicationStatus::DelegatingDeployment);
                                deployment.commit = commit;
                                if let Some(c) = deployment_config {
                                    deployment.port = format!("{}", c.config.port);
                                }
                                self.update_deployment_in_db(&deployment);
                                let nodes = super::do_db_task(self, |db| -> Option<Vec<Node>> {
                                    db.get_nodes()
                                });
                                info!("{:?}", nodes);
                                match nodes {
                                    None => {
                                        warn!(
                                            "No node available to accept deployment {}",
                                            &deployment.id
                                        );
                                        deployment.update_status(&ApplicationStatus::Errored);
                                        self.update_deployment_in_db(&deployment);
                                    }
                                    Some(nodes) => {
                                        let mut curr_node = &nodes[0];
                                        for node in nodes.iter() {
                                            // For now, pick the node with the fewest application instances
                                            // TODO make this process smart
                                            if node.deployments.len() < curr_node.deployments.len()
                                            {
                                                curr_node = node;
                                            }
                                        }

                                        deployment.node = curr_node.id.clone();
                                        deployment.update_status(&ApplicationStatus::Errored);
                                        self.update_deployment_in_db(&deployment);
                                        // Send work to curr_node
                                        let msg = WorkRequestMessage::new(
                                            WorkRequestType::RequestDeployment,
                                            Some(&deployment.id),
                                            Some(&deployment.src_url),
                                            Some(&deployment.git_branch),
                                            None,
                                        );
                                        let publisher =
                                            node.broker.as_ref().unwrap().get_channel().await;
                                        msg.send(&publisher, &curr_node.id).await;

                                        super::do_db_task(self, |db| {
                                            db.add_deployment_to_node(
                                                &curr_node.id,
                                                deployment.id.clone(),
                                            )
                                            .unwrap();
                                        });
                                        // Deployment has been scheduled
                                    }
                                }
                            }
                        }
                    }
                    ApplicationStatus::UpdateRequested => {
                        let commit = super::github_api::get_tail_commit_for_branch_from_url(
                            &deployment.git_branch,
                            &deployment.src_url,
                        )
                        .await;
                        match commit {
                            None => {}
                            Some(c) => {
                                if c != deployment.commit {
                                    // Need to redeploy
                                    info!(
                                        "Update on remote for deployment {} detected, redeploying",
                                        &deployment.id
                                    );
                                    deployment
                                        .update_status(&ApplicationStatus::DelegatingDestruction);
                                    self.update_deployment_in_db(&deployment);
                                    let msg = WorkRequestMessage::new(
                                        WorkRequestType::CancelDeployment,
                                        Some(&deployment.id),
                                        Some(&deployment.git_branch),
                                        None,
                                        None,
                                    );
                                    let publisher =
                                        node.broker.as_ref().unwrap().get_channel().await;
                                    msg.send(&publisher, &deployment.node).await;
                                    let msg = WorkRequestMessage::new(
                                        WorkRequestType::RequestDeployment,
                                        Some(&deployment.id),
                                        Some(&deployment.src_url),
                                        Some(&deployment.git_branch),
                                        None,
                                    );
                                    msg.send(&publisher, &deployment.node).await;
                                } else {
                                    info!("Update requested for up-to-date deployment");
                                    deployment.update_status(&ApplicationStatus::Running);
                                    self.update_deployment_in_db(&deployment);
                                }
                            }
                        }
                    }
                    ApplicationStatus::DestructionRequested => {
                        deployment.update_status(&ApplicationStatus::DelegatingDestruction);
                        super::do_db_task(self, |db| {
                            db.update_deployment(&deployment.id, &deployment);
                            db.remove_deployment_from_nodes(&deployment.id);
                        });
                        let msg = WorkRequestMessage::new(
                            WorkRequestType::CancelDeployment,
                            Some(&deployment.id),
                            None,
                            None,
                            None,
                        );
                        let publisher = node.broker.as_ref().unwrap().get_channel().await;
                        msg.send(&publisher, &deployment.node).await;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
