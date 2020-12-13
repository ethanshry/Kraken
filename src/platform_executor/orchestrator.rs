//! Defines the Orchestrator role, which manages work for all devices on the platform
use crate::db::{Database, ManagedDatabase};
use crate::file_utils::{append_to_file, clear_tmp, copy_dir_contents_to_static};
use crate::git_utils::clone_remote_branch;
use crate::gql_model::{ApplicationStatus, Node, Service, ServiceStatus};
use crate::gql_schema::{Mutation, Query};
use crate::network::wait_for_good_healthcheck;
use crate::platform_executor::{ExecuteFaliure, GenericNode, SetupFaliure, Task};
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    log_message::LogMessage,
    sysinfo_message::SysinfoMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use futures::{future, FutureExt};
use log::{info, warn};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Only one device on the network has the Orchestrator role at a single time
/// Handles coordination of all other nodes
pub struct Orchestrator {
    /// A handle to the rocket.rs http server task
    api_server: Option<tokio::task::JoinHandle<()>>,
    /// A reference to the Database containing information about the platform
    pub db_ref: Arc<Mutex<Database>>,
}

/// Spins up a Rocket API Server for the orchestration node
pub fn create_api_server(o: &Orchestrator) -> tokio::task::JoinHandle<()> {
    let options = rocket_cors::CorsOptions {
        ..Default::default()
    }
    .to_cors()
    .unwrap();

    let db_ref = o.db_ref.clone();

    // launch rocket
    tokio::spawn(async move {
        rocket::ignite()
            .manage(ManagedDatabase::new(db_ref))
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
                    crate::api_routes::logs
                ],
            )
            .attach(options)
            .launch();
    })
}

/// Pulls the Kraken-UI to be served by the API Server
async fn fetch_ui(o: &Orchestrator) {
    if &std::env::var("SHOULD_CLONE_UI").unwrap_or_else(|_| "YES".into())[..] == "NO" {
        warn!(
            "ENV is configured to skip UI download and compile, this may not be desired behaviour"
        );
        return;
    }

    let git_data =
        crate::github_api::get_tail_commits_for_repo_branches("ethanshry", "kraken-ui").await;

    let mut sha = String::from("");

    let should_update_ui = match git_data {
        Some(data) => {
            let mut flag = true;
            for branch in data {
                if branch.name == "build" {
                    let arc = o.db_ref.clone();
                    let db = arc.lock().unwrap();
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
            "https://github.com/ethanshry/Kraken-UI.git",
            "build",
            "tmp/site",
        )
        .wait()
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
/// TODO unbind this from the makefile (#52)
pub async fn deploy_rabbit_instance(node: &GenericNode, o: &Orchestrator) -> Result<(), String> {
    match Command::new("make").arg("spinup-rabbit").output() {
        Ok(_) => {
            || -> () {
                let arc = o.db_ref.clone();
                let mut db = arc.lock().unwrap();
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

/// Ensures the deployment has the potential for success
/// This includes ensuring the url is valid and the shipwreck.toml exists in the repo
pub async fn validate_deployment(git_url: &str) -> Result<String, ()> {
    match crate::github_api::parse_git_url(git_url) {
        None => Err(()),
        Some(url_data) => {
            match crate::github_api::check_for_file_in_repo(
                &url_data.user,
                &url_data.repo,
                "shipwreck.toml",
            )
            .await
            {
                Some(_) => {
                    let commits = crate::github_api::get_tail_commits_for_repo_branches(
                        &url_data.user,
                        &url_data.repo,
                    )
                    .await;
                    match commits {
                        None => Err(()),
                        Some(commits) => {
                            for c in commits {
                                // TODO fix so any branch is accepted (#53)
                                // Allow legacy 'master' branches
                                if c.name == "master" || c.name == "main" {
                                    return Ok(c.commit.sha);
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

impl Orchestrator {
    /// Creates a new Orchestrator Object
    pub fn new() -> Orchestrator {
        let db = Database::new();
        Orchestrator {
            api_server: None,
            db_ref: Arc::new(Mutex::new(db)),
        }
    }
}

/// The tasks associated with setting up this role.
/// For an orchestrator, this involves setting up the RabbitMQ server, as well as the rocket.rs server, and establishing queue consumers for messages
pub async fn setup(node: &mut GenericNode, o: &mut Orchestrator) -> Result<(), SetupFaliure> {
    let arc = o.db_ref.clone();
    let mut db = arc.lock().unwrap();
    db.insert_node(&Node::new(
        &node.system_id,
        "Placeholder Model",
        0,
        0,
        0,
        0.0,
    ));
    drop(db);

    // clear all tmp files
    clear_tmp();

    fs::create_dir_all("log").unwrap();

    // download site
    let ui = fetch_ui(o);

    let rabbit = deploy_rabbit_instance(&node, &o);

    let tasks = future::join(ui, rabbit);
    let (_ui_res, _rabbit_res) = tasks.await;

    // Validate connection is possible
    match crate::platform_executor::connect_to_rabbit_instance(&node.rabbit_addr).await {
        Ok(b) => node.broker = Some(b),
        Err(e) => {
            warn!("{}", e);
            return Err(SetupFaliure::NoRabbit);
        }
    }

    info!("Succesfully established RabbitMQ service");

    // Unwrap is safe here because we will have returned Err if broker is None
    node.broker
        .as_ref()
        .unwrap()
        .declare_queue(QueueLabel::Sysinfo.as_str())
        .await;

    node.broker
        .as_ref()
        .unwrap()
        .declare_queue(QueueLabel::Deployment.as_str())
        .await;

    node.broker
        .as_ref()
        .unwrap()
        .declare_queue(QueueLabel::Log.as_str())
        .await;

    // consume node information queue(s)
    let sysinfo_consumer = {
        let arc = o.db_ref.clone();
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (node, message) = SysinfoMessage::deconstruct_message(&data);
            // TODO clean up all this unsafe unwrapping
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
            let broker = match crate::platform_executor::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Sysinfo.as_str(), &handler)
                .await;
        })
    };
    node.queue_consumers.push(Task {
        task: sysinfo_consumer,
        label: String::from(QueueLabel::Sysinfo.as_str()),
    });

    // consume deployment status queue
    let deployment_consumer = {
        let arc = o.db_ref.clone();

        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (node, message) = DeploymentMessage::deconstruct_message(&data);

            // TODO add info to the db
            let mut db = arc.lock().unwrap();
            let deployment = db.get_deployment(&message.deployment_id);
            match deployment {
                Some(d) => {
                    let mut updated_deployment = d.clone();
                    updated_deployment.update_status(message.deployment_status.clone());
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
            let broker = match crate::platform_executor::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Deployment.as_str(), &handler)
                .await;
        })
    };
    node.queue_consumers.push(Task {
        task: deployment_consumer,
        label: String::from(QueueLabel::Deployment.as_str()),
    });

    // consume deployment log queue
    let log_consumer = {
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (deployment_id, message) = LogMessage::deconstruct_message(&data);
            append_to_file(&format!("log/{}.log", &deployment_id), &message.message);
        };
        tokio::spawn(async move {
            let broker = match crate::platform_executor::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(_) => panic!("Could not establish rabbit connection"),
            };
            broker
                .consume_queue(&system_uuid, QueueLabel::Log.as_str(), &handler)
                .await;
        })
    };
    node.queue_consumers.push(Task {
        task: log_consumer,
        label: String::from(QueueLabel::Log.as_str()),
    });

    o.api_server = Some(create_api_server(o));
    Ok(())
}

/// Logic which should be executed every iteration
/// Orchestrators are primarilly focused on determing if deployment requests are valid, and distreibuting them to the best-suited worker device
pub async fn execute(node: &GenericNode, o: &Orchestrator) -> Result<(), ExecuteFaliure> {
    // Todo look for work to distribute and do it
    async || -> () {
        // Look through deployments for new deployments which need to be scheduled
        let arc = o.db_ref.clone();
        let db = arc.lock().unwrap();
        let deployments = db.get_deployments();
        drop(db);
        if let Some(d) = deployments {
            for mut deployment in d {
                match deployment.status.0 {
                    ApplicationStatus::DeploymentRequested => {
                        // Look for free nodes to distribute tasks to
                        deployment.update_status(ApplicationStatus::ValidatingDeploymentData);
                        let mut db = arc.lock().unwrap();
                        db.update_deployment(&deployment.id, &deployment);
                        drop(db);

                        match validate_deployment(&deployment.src_url).await {
                            Err(_) => {
                                warn!("Deployment failed validation {}", &deployment.id);
                                let mut db = arc.lock().unwrap();
                                deployment.update_status(ApplicationStatus::Errored);
                                db.update_deployment(&deployment.id, &deployment);
                                drop(db);
                            }
                            Ok(commit) => {
                                let mut db = arc.lock().unwrap();
                                deployment.update_status(ApplicationStatus::DelegatingDeployment);
                                deployment.commit = commit;
                                db.update_deployment(&deployment.id, &deployment);
                                let nodes = db.get_nodes();
                                info!("{:?}", nodes);
                                match nodes {
                                    None => {
                                        warn!(
                                            "No node available to accept deployment {}",
                                            &deployment.id
                                        );
                                        deployment.update_status(ApplicationStatus::Errored);
                                        db.update_deployment(&deployment.id, &deployment);
                                    }
                                    Some(nodes) => {
                                        let mut curr_node = &nodes[0];
                                        for node in nodes.iter() {
                                            // For now, pick the node with the fewest application instances
                                            // TODO make this process smart
                                            if node.application_instances.len()
                                                < curr_node.application_instances.len()
                                            {
                                                curr_node = node;
                                            }
                                        }

                                        deployment.node = curr_node.id.clone();
                                        deployment.update_status(ApplicationStatus::Errored);
                                        db.update_deployment(&deployment.id, &deployment);

                                        // curr_node will recieve the work

                                        // Send work to curr_node
                                        let msg = WorkRequestMessage::new(
                                            WorkRequestType::RequestDeployment,
                                            Some(&deployment.id),
                                            Some(&deployment.src_url),
                                            None,
                                        );
                                        let publisher =
                                            node.broker.as_ref().unwrap().get_channel().await;
                                        msg.send(&publisher, &curr_node.id).await;

                                        db.add_application_instance_to_node(
                                            &curr_node.id,
                                            deployment.id,
                                        )
                                        .unwrap();
                                        // Deployment has been scheduled
                                    }
                                }
                            }
                        }
                    }
                    ApplicationStatus::UpdateRequested => {
                        let commit = crate::github_api::get_tail_commit_for_branch_from_url(
                            "main",
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
                                    let mut db = arc.lock().unwrap();
                                    deployment
                                        .update_status(ApplicationStatus::DelegatingDestruction);
                                    db.update_deployment(&deployment.id, &deployment);
                                    drop(db);
                                    let msg = WorkRequestMessage::new(
                                        WorkRequestType::CancelDeployment,
                                        Some(&deployment.id),
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
                                        None,
                                    );
                                    msg.send(&publisher, &deployment.node).await;
                                } else {
                                    info!("Update requested for up-to-date deployment");
                                    let mut db = arc.lock().unwrap();
                                    deployment.update_status(ApplicationStatus::Running);
                                    db.update_deployment(&deployment.id, &deployment);
                                    drop(db);
                                }
                            }
                        }
                    }
                    ApplicationStatus::DestructionRequested => {
                        let mut db = arc.lock().unwrap();
                        deployment.update_status(ApplicationStatus::DelegatingDestruction);
                        db.update_deployment(&deployment.id, &deployment);
                        db.remove_application_instance_from_nodes(&deployment.id);
                        drop(db);
                        let msg = WorkRequestMessage::new(
                            WorkRequestType::CancelDeployment,
                            Some(&deployment.id),
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
    }()
    .await;
    Ok(())
}
