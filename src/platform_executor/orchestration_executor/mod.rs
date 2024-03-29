//! Defines the Orchestrator role, which manages work for all devices on the platform
use super::{ExecutionFailure, Executor, GenericNode, SetupFailure, Task};
use crate::gql_model::{Deployment, Service, ServiceStatus};
use crate::gql_schema::{Mutation, Query};
use crate::rabbit::RabbitBroker;
use crate::rabbit::{
    deployment_message::DeploymentMessage, log_message::LogMessage,
    sysinfo_message::SysinfoMessage, QueueLabel, RabbitMessage,
};
use kraken_utils::file::{append_to_file, copy_dir_contents_to_static, overwrite_to_file};
use kraken_utils::git::clone_remote_branch;
use kraken_utils::network::wait_for_good_healthcheck;
use log::{info, warn};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::{fs, str};
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
    /// The rank of this executor for rollover. 0 implies this is the active orchestrator, None implies none is assigned.
    /// Otherwise is treated as lowest number is highest priority
    pub rollover_priority: Option<u8>,
    pub queue_consumers: Vec<Task>,
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

/// Attempts to fetch the numeric priority for the specified system in an orchestration failure
///
/// # Arguments
///
/// * `orchestrator_addr` - The ip and port of the currently active orchestrator
/// * `system_id` - The node id which we are trying to request the priority for
///
/// # Returns
///
/// * Some(u8) - The rollover priority for the specified system_id
/// * None - No priority indicates there was an issue communicating with the primary orchestrator
///
pub async fn get_rollover_priority(orchestrator_addr: &str, system_id: &str) -> Option<u8> {
    let url = format!(
        "http://{orchestrator_addr}/health/{node_id}",
        orchestrator_addr = orchestrator_addr,
        node_id = system_id
    );

    info!("Making request to: {}", url);

    let client = reqwest::Client::new();

    let response = client.get(&url).send().await;

    match response {
        Ok(r) => match r.text().await {
            Ok(data) => Some(data.parse::<u8>().unwrap()),
            Err(e) => {
                info!("Failed to parse response: {}", e);
                None
            }
        },
        Err(_) => None,
    }
}

/// Requests a copy of the Database data from the orchestrator_addr, and stores it in the Database on the current Node
pub async fn get_db_data(orchestrator_addr: &str) -> Option<Database> {
    let url = format!(
        "http://{orchestrator_addr}/export/database",
        orchestrator_addr = orchestrator_addr,
    );

    info!("Making request to: {}", url);

    let client = reqwest::Client::new();

    let response = client.get(&url).send().await;

    match response {
        Ok(r) => match r.json::<Database>().await {
            Ok(data) => Some(data),
            Err(e) => {
                info!("Failed to parse JSON to Database: {}", e);
                None
            }
        },
        Err(_) => None,
    }
}

/// Requests a copy of the specified logfile from the orchestrator_addr, and stores it in the appropriate location on the current Node
pub async fn backup_log_file(orchestrator_addr: &str, log_id: &str) {
    let url = format!(
        "http://{orchestrator_addr}/log/{log_id}",
        orchestrator_addr = orchestrator_addr,
        log_id = log_id
    );

    info!("Making request to: {}", url);

    let client = reqwest::Client::new();

    let response = client.get(&url).send().await;

    if let Ok(r) = response {
        match r.text().await {
            Ok(data) => {
                // write log to file
                overwrite_to_file(
                    &format!("{}/{}.log", crate::utils::LOG_LOCATION, log_id),
                    &data,
                );
            }
            Err(e) => {
                info!("Failed to parse logfile response: {}", e);
            }
        }
    }
}

impl OrchestrationExecutor {
    /// Creates a new Orchestrator Object
    pub fn new(rollover_priority: Option<u8>) -> OrchestrationExecutor {
        let db = Database::new();
        OrchestrationExecutor {
            api_server: None,
            db_ref: Arc::new(Mutex::new(db)),
            rollover_priority: rollover_priority,
            queue_consumers: vec![],
        }
    }

    /// Pulls the Kraken-UI to be served by the API Server
    ///
    /// # Arguments
    ///
    /// * `db` - An Arc to a Database where we can store information about the UI
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

            // Inject github token to site
            kraken_utils::file::append_to_file(
                "tmp/site/.env",
                &format!(
                    "AUTH={}",
                    std::env::var("GITHUB_TOKEN").unwrap_or(String::from("undefined"))
                ),
            );

            // Build the site
            info!("Compiling Kraken-UI");
            let output = std::process::Command::new("npm")
                .arg("run")
                .arg("build")
                .current_dir("tmp/site")
                .output()
                .expect("err in clone");

            let data = str::from_utf8(&output.stderr).unwrap();

            kraken_utils::file::append_to_file("tmp/site/out.txt", data);
            copy_dir_contents_to_static("tmp/site/public");
            copy_dir_contents_to_static("tmp/site/dist");
            fs::remove_dir_all("tmp/site").unwrap();
            info!("Compilation of Kraken-UI Complete");
        }

        info!("Kraken-UI is now available at commit SHA: {}", sha);
    }

    /// Cleans up all running docker containers
    pub async fn clean_docker() {
        match Command::new("make").arg("cleanup").output() {
            Ok(_) => info!("System has sucesfully cleaned up all docker images"),
            Err(_) => warn!("System failed to clean up docker images. Usually this means there was nothing to clean up")
        };
    }

    /// Deploys a new RabbitMQ instance to the local machine
    ///
    /// # Arguments
    ///
    /// * `node` - A GenericNode containing information about the platform
    /// * `db` - An Arc to a Database which we can store information about our connection
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
                .manage(crate::api_routes::Schema::new(Query, Mutation))
                .mount(
                    "/",
                    rocket::routes![
                        crate::api_routes::root,
                        crate::api_routes::ping,
                        crate::api_routes::graphiql,
                        crate::api_routes::get_graphql_handler,
                        crate::api_routes::post_graphql_handler,
                        crate::api_routes::site,
                        crate::api_routes::logs,
                        crate::api_routes::health,
                        crate::api_routes::export_db
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

    /// declares the queues in RabbitMQ which the orchestrator will be consuming
    async fn establish_rabbit_queues(&mut self, broker: &RabbitBroker) -> Result<(), ()> {
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
