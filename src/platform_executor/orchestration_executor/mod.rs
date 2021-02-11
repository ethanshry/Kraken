//! Defines the Orchestrator role, which manages work for all devices on the platform
use super::{ExecutionFaliure, Executor, GenericNode, SetupFaliure, Task};
use crate::gql_model::{Deployment, Service, ServiceStatus};
use crate::gql_schema::{Mutation, Query};
use crate::rabbit::RabbitBroker;
use crate::rabbit::{
    deployment_message::DeploymentMessage, log_message::LogMessage,
    sysinfo_message::SysinfoMessage, QueueLabel, RabbitMessage,
};
use juniper::EmptySubscription;
use kraken_utils::file::{append_to_file, copy_dir_contents_to_static};
use kraken_utils::git::clone_remote_branch;
use kraken_utils::network::wait_for_good_healthcheck;
use log::{info, warn};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::task::JoinHandle;

pub mod db;
mod executor;
pub mod github_api;

use db::{Database, ManagedDatabase};

/// Only one device on the network has the Orchestrator role at a single time
/// Handles coordination of all other nodes
pub struct OrchestrationExecutor {
    /// A handle to the rocket.rs http server task
    api_server: Option<tokio::task::JoinHandle<()>>,
    /// A reference to the Database containing information about the platform
    pub db_ref: Arc<Mutex<Database>>,
}

/// Ensures the deployment has the potential for success
/// This includes ensuring the url is valid and the shipwreck.toml exists in the repo
///
/// # Arguments
///
/// * `git_url` - The URL to a github repository containing a shipwreck.toml
///
/// # Returns
///
/// * Ok((commit_hash, shipwreck.toml data))
/// * Err(())
///
/// # Examples
/// ```
/// let url = validate_deployment("http://github.com/Kraken/scapenode", "main")
/// assert_eq!(url, Ok(_));
/// ```
pub async fn validate_deployment(git_url: &str, git_branch: &str) -> Result<(String, String), ()> {
    match github_api::parse_git_url(git_url) {
        None => Err(()),
        Some(url_data) => {
            match github_api::check_for_file_in_repo(
                &url_data.user,
                &url_data.repo,
                git_branch,
                "shipwreck.toml",
            )
            .await
            {
                Some(shipwreck_data) => {
                    let commits = github_api::get_tail_commits_for_repo_branches(
                        &url_data.user,
                        &url_data.repo,
                    )
                    .await;
                    match commits {
                        None => Err(()),
                        Some(commits) => {
                            for c in commits {
                                if c.name == git_branch {
                                    return Ok((c.commit.sha, shipwreck_data));
                                }
                            }
                            Err(())
                        }
                    }
                }
                None => Err(()),
            }
        }
    }
}

impl OrchestrationExecutor {
    /// Creates a new Orchestrator Object
    pub fn new() -> OrchestrationExecutor {
        let db = Database::new();
        OrchestrationExecutor {
            api_server: None,
            db_ref: Arc::new(Mutex::new(db)),
        }
    }

    /// Pulls the Kraken-UI to be served by the API Server
    ///
    /// # Arguments
    ///
    /// * `o` - An OrchestrationExecutor with a reference to the database
    async fn fetch_ui(db: Arc<Mutex<Database>>) {
        if &std::env::var("SHOULD_CLONE_UI").unwrap_or_else(|_| "YES".into())[..] == "NO" {
            warn!(
            "ENV is configured to skip UI download and compile, this may not be desired behaviour"
        );
            return;
        }

        let git_data =
            github_api::get_tail_commits_for_repo_branches("ethanshry", "kraken-ui").await;

        let mut sha = String::from("");

        let should_update_ui = match git_data {
            Some(data) => {
                let mut flag = true;
                for branch in data {
                    if branch.name == crate::utils::UI_BRANCH_NAME {
                        let db = db.lock().unwrap();
                        let active_branch = db.get_orchestrator().ui.cloned_commit;
                        // unlock db
                        drop(db);
                        match active_branch {
                            Some(b) => {
                                if branch.commit.sha == b {
                                    flag = false;
                                    sha = b.clone();
                                } else {
                                    sha = branch.commit.sha;
                                }
                            }
                            None => {
                                sha = branch.commit.sha.clone();
                            }
                        }
                        break;
                    }
                }
                flag
            }
            None => true,
        };

        if sha == "" {
            info!("No build branch found, cannot update kraken-ui");
            return;
        }

        if should_update_ui {
            info!("Cloning updated UI...");
            clone_remote_branch(
                crate::utils::UI_GIT_ADDR,
                crate::utils::UI_BRANCH_NAME,
                "tmp/site",
            )
            .unwrap();

            // Build the site
            info!("Compiling Kraken-UI");
            std::process::Command::new("npm")
                .arg("run")
                .arg("build")
                .current_dir("tmp/site")
                .output()
                .expect("err in clone");

            copy_dir_contents_to_static("tmp/site/public");
            copy_dir_contents_to_static("tmp/site/dist");
            fs::remove_dir_all("tmp/site").unwrap();
            info!("Compilation of Kraken-UI Complete");
        }

        info!("Kraken-UI is now available at commit SHA: {}", sha);
    }

    /// Deploys a new RabbitMQ instance to the local machine
    ///
    /// # Arguments
    ///
    /// * `node` - A GenericNode containing information about the platform
    /// * `o` - An OrchestrationExecutor with a reference to the database
    pub async fn deploy_rabbit_instance(
        db: Arc<Mutex<Database>>,
        node: &GenericNode,
    ) -> Result<(), String> {
        match Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--hostname")
            .arg("rabbitmq.service.dev")
            .arg("-p")
            .arg("5672:5672")
            .arg("-p")
            .arg("15672:15672")
            .arg("rabbitmq:3-management")
            .output()
        {
            Ok(_) => {
                || -> () {
                    let mut db = db.lock().unwrap();
                    db.add_platform_service(Service::new(
                        "rabbitmq",
                        "0.1.0",
                        &node.rabbit_addr,
                        ServiceStatus::OK,
                    ))
                    .unwrap();
                }();
                info!("Waiting for RabbitMQ server to spinup...");
                match wait_for_good_healthcheck("http://localhost:15672", None).await {
                    true => {
                        std::thread::sleep(std::time::Duration::new(1, 0));
                        Ok(())
                    }
                    false => Err(String::from("Failed to satisfy healthcheck")),
                }
            }
            Err(_) => Err(String::from("Failed to create rabbit instance")),
        }
    }

    /// Spins up a Rocket API Server for the orchestration node
    pub fn create_api_server(db: Arc<Mutex<Database>>) -> tokio::task::JoinHandle<()> {
        let options = rocket_cors::CorsOptions {
            ..Default::default()
        }
        .to_cors()
        .unwrap();

        // launch rocket
        tokio::spawn(async move {
            rocket::ignite()
                .manage(ManagedDatabase::new(db))
                .manage(crate::api_routes::Schema::new(
                    Query,
                    Mutation,
                    EmptySubscription::<ManagedDatabase>::new(),
                ))
                .mount(
                    "/",
                    rocket::routes![
                        crate::api_routes::root,
                        crate::api_routes::ping,
                        crate::api_routes::graphiql,
                        crate::api_routes::get_graphql_handler,
                        crate::api_routes::post_graphql_handler,
                        crate::api_routes::site,
                        crate::api_routes::logs
                    ],
                )
                .attach(options)
                .launch();
        })
    }

    /// A wrapper around the do_db_task helper
    /// Strictly updates the db Deplopyment to match the local one
    fn update_deployment_in_db(&mut self, deployment: &Deployment) {
        do_db_task(self, |db| {
            db.update_deployment(&deployment.id, &deployment);
        });
    }

    /// Establishes the thread to consume sysinfo messages
    fn get_sysinfo_consumer(&mut self, node: &mut GenericNode) -> JoinHandle<()> {
        let arc = self.db_ref.clone();
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (node, message) = SysinfoMessage::deconstruct_message(&data);
            let mut db = arc.lock().unwrap();
            if let Some(n) = db.get_node(&node) {
                let mut new_node = n;
                new_node.set_id(&node);
                new_node.update(
                    message.ram_free,
                    message.ram_used,
                    message.uptime,
                    message.load_avg_5,
                );
                db.insert_node(&new_node);
            } else {
                let new_node = crate::gql_model::Node::from_incomplete(
                    &node,
                    None,
                    &message.lan_addr,
                    Some(message.ram_free),
                    Some(message.ram_used),
                    Some(message.uptime),
                    Some(message.load_avg_5),
                    None,
                    None,
                );
                db.insert_node(&new_node);
            }
        };
        tokio::spawn(async move {
            let broker = match Self::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Sysinfo.as_str(), &handler)
                .await;
        })
    }

    /// Establishes the thread to consume deployment messages
    fn get_deployment_consumer(&mut self, node: &mut GenericNode) -> JoinHandle<()> {
        let arc = self.db_ref.clone();

        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (node, message) = DeploymentMessage::deconstruct_message(&data);

            let mut db = arc.lock().unwrap();
            let deployment = db.get_deployment(&message.deployment_id);
            match deployment {
                Some(d) => {
                    let mut updated_deployment = d.clone();
                    updated_deployment.update_status(&message.deployment_status.clone());
                    if let Some(status) = message.container_status {
                        updated_deployment.update_container_status(&status);
                    }
                    db.update_deployment(&message.deployment_id, &updated_deployment);
                    info!(
                        "{} : Deployment {}\n\t{} : {}",
                        node,
                        message.deployment_id,
                        message.deployment_status,
                        message.deployment_status_description
                    );
                }
                None => {
                    warn!("Recieved the following deployment info for a deployment which does not exist in the database:\n{} : Deployment {}\n\t{} : {}", node,
                    message.deployment_id,
                    message.deployment_status,
                    message.deployment_status_description);
                }
            }
        };

        tokio::spawn(async move {
            let broker = match Self::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Deployment.as_str(), &handler)
                .await;
        })
    }

    /// Establishes the thread to consume log messages
    fn get_log_consumer(&mut self, node: &mut GenericNode) -> JoinHandle<()> {
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (deployment_id, message) = LogMessage::deconstruct_message(&data);
            append_to_file(
                &format!("{}/{}.log", crate::utils::LOG_LOCATION, &deployment_id),
                &message.message,
            );
        };
        tokio::spawn(async move {
            let broker = match Self::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Log.as_str(), &handler)
                .await;
        })
    }

    async fn establish_rabbit_queues(&mut self, broker: &RabbitBroker) -> Result<(), ()> {
        // Unwrap is safe here because we will have returned Err if broker is None
        let mut result = false || broker.declare_queue(QueueLabel::Sysinfo.as_str()).await;

        result |= broker.declare_queue(QueueLabel::Deployment.as_str()).await;

        result |= broker.declare_queue(QueueLabel::Log.as_str()).await;

        match result {
            true => Ok(()),
            false => Err(()),
        }
    }
}

/// Provides a closure direct access to the database through the Arc<Mutex>>
///
/// # Arguments
///
/// * `executor` - The OrchestrationExecutor that owns an Arc to the Database
/// * `handle` - A closure which accepts a T, and returns a U
/// where `T` is a mutable reference to the database, and `U` is the return type of the closure
/// # Examples
///
/// Example with no return type
/// ```
/// let orchestrator = platform_executor::orchestration_executor::OrchestrationExecutor::new();
/// do_db_task(&mut orchestrator, |db| {
///     let nodes = db.get_nodes();
///     assert_eq!(nodes, None);
/// });
/// ```
/// Example with return type
/// ```
/// let orchestrator = platform_executor::orchestration_executor::OrchestrationExecutor::new();
/// let nodes = do_db_task(&mut orchestrator, |db| -> Option<Vec<Nodes>> {
///     db.get_nodes()
/// });
/// assert_eq!(nodes, None);
/// ```
fn do_db_task<T, U>(executor: &mut OrchestrationExecutor, handle: T) -> U
where
    T: Fn(&mut MutexGuard<'_, Database>) -> U,
{
    let arc = executor.db_ref.clone();
    let mut db = arc.lock().unwrap();
    handle(&mut db)
}
