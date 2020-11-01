use crate::db::{Database, ManagedDatabase};
use crate::file_utils::{append_to_file, clear_tmp, copy_dir_contents_to_static};
use crate::git_utils::clone_remote_branch;
use crate::model::{ApplicationStatus, Node, Service, ServiceStatus};
use crate::platform_executor::{GenericNode, SetupFaliure, Task, TaskFaliure};
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    log_message::LogMessage,
    sysinfo_message::SysinfoMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use crate::schema::{Mutation, Query};
use log::{info, warn};
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};

pub struct Orchestrator {
    api_server: Option<tokio::task::JoinHandle<()>>,
    //database: Database,
    pub db_ref: Arc<Mutex<Database>>,
}

// TODO add broker to utils

/// Spins up a Rocket API Server for the orchestration node
pub fn create_api_server(o: &Orchestrator) -> tokio::task::JoinHandle<()> {
    let options = rocket_cors::CorsOptions {
        ..Default::default()
    }
    .to_cors()
    .unwrap();

    let db_ref = o.db_ref.clone();

    // launch rocket
    let server = tokio::spawn(async move {
        rocket::ignite()
            .manage(ManagedDatabase::new(db_ref))
            .manage(crate::routes::Schema::new(Query, Mutation))
            .mount(
                "/",
                rocket::routes![
                    crate::routes::root,
                    crate::routes::ping,
                    crate::routes::graphiql,
                    crate::routes::get_graphql_handler,
                    crate::routes::post_graphql_handler,
                    crate::routes::site
                ],
            )
            .attach(options)
            .launch();
    });
    server
}

/// Pulls the Kraken-UI to be served by the API Server
async fn fetch_ui(o: &Orchestrator) -> () {
    let git_data =
        crate::gitapi::GitApi::get_tail_commits_for_repo_branches("ethanshry", "kraken-ui").await;

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

        copy_dir_contents_to_static("tmp/site/dist");
        fs::remove_dir_all("tmp/site").unwrap();
        info!("Compilation of Kraken-UI Complete");
    }

    info!("Kraken-UI is now available at commit SHA: {}", sha);
}

// Deploys a new RabbitMQ instance to the local machine
pub fn deploy_rabbit_instance(node: &GenericNode, o: &Orchestrator) -> Result<(), String> {
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
            // TODO figure out how to be better- somehow, rabbit is not OK as soon as docker has spun up
            std::thread::sleep(std::time::Duration::new(15, 0));
            Ok(())
        }
        Err(_) => Err(String::from("Failed to create rabbit instance")),
    }
}

/// Ensures the deployment has the potential for success
/// This includes ensuring the url is valid and the shipwreck.toml exists in the repo
pub async fn validate_deployment(git_url: &str) -> Result<String, ()> {
    // https://github.com/ethanshry/scapegoat
    match crate::gitapi::GitApi::parse_git_url(git_url) {
        None => Err(()),
        Some(url_data) => {
            match crate::gitapi::GitApi::check_for_file_in_repo(
                &url_data.user,
                &url_data.repo,
                "shipwreck.toml",
            )
            .await
            {
                Some(_) => {
                    let commits = crate::gitapi::GitApi::get_tail_commits_for_repo_branches(
                        &url_data.user,
                        &url_data.repo,
                    )
                    .await;
                    match commits {
                        None => Err(()),
                        Some(commits) => {
                            for c in commits {
                                // TODO fix so any branch is accepted
                                // Allow legacy 'master' branches
                                if c.name == "master" || c.name == "main" {
                                    return Ok(c.commit.sha.to_string());
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

pub async fn setup(node: &mut GenericNode, o: &mut Orchestrator) -> Result<(), SetupFaliure> {
    // TODO load platform information from a save state
    /*let platform: Platform =
        crate::utils::load_or_create_platform(&mut o.database.as_ref().unwrap());

    info!("Platform: {:?}", platform);
    */
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

    fs::create_dir_all("log/.kraken").unwrap();

    fs::create_dir_all("log/.kraken/logs").unwrap();

    // download site
    let ui = fetch_ui(o);

    // Establish RabbitMQ
    if let Err(e) = deploy_rabbit_instance(&node, &o) {
        warn!("{}", e);
        return Err(SetupFaliure::BadRabbit);
    }

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

    // TODO setup dns records

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
                let mut new_node = n.clone();
                new_node.set_id(&node);
                new_node.update(
                    message.ram_free,
                    message.ram_used,
                    message.uptime,
                    message.load_avg_5,
                );
                db.insert_node(&new_node);
            } else {
                let new_node = crate::model::Node::from_incomplete(
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
            return ();
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

            return ();
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
            append_to_file(
                &format!("log/.kraken/logs/{}.log", &deployment_id),
                &message.message,
            );
            return ();
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

    // You can also deserialize this

    // monitor git

    // deploy docker services if determined to be best practice

    ui.await;

    o.api_server = Some(create_api_server(o));
    Ok(())
}

pub async fn execute(node: &GenericNode, o: &Orchestrator) -> Result<(), TaskFaliure> {
    // Todo look for work to distribute and do it
    async || -> () {
        // Look through deployments for new deployments which need to be scheduled
        let arc = o.db_ref.clone();
        let db = arc.lock().unwrap();
        let deployments = db.get_deployments();
        drop(db);
        if let Some(d) = deployments {
            for mut deployment in d {
                match deployment.status {
                    ApplicationStatus::DeploymentRequested => {
                        // Look for free nodes to distribute tasks to
                        deployment.status = ApplicationStatus::ValidatingDeploymentData;
                        let mut db = arc.lock().unwrap();
                        db.update_deployment(&deployment.id, &deployment);
                        drop(db);

                        match validate_deployment(&deployment.src_url).await {
                            Err(_) => {
                                warn!("Deployment failed validation {}", &deployment.id);
                                let mut db = arc.lock().unwrap();
                                deployment.status = ApplicationStatus::Errored;
                                db.update_deployment(&deployment.id, &deployment);
                                drop(db);
                            }
                            Ok(commit) => {
                                let mut db = arc.lock().unwrap();
                                deployment.status = ApplicationStatus::DelegatingDeployment;
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
                                        deployment.status = ApplicationStatus::Errored;
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
                                        deployment.status = ApplicationStatus::Errored;
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
                                        msg.send(&publisher, &node.system_id).await;

                                        // Deployment has been scheduled
                                    }
                                }
                            }
                        }
                    }
                    ApplicationStatus::UpdateRequested => {
                        let commit = crate::gitapi::GitApi::get_tail_commit_for_branch_from_url(
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
                                    deployment.status = ApplicationStatus::DelegatingDestruction;
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
                                    deployment.status = ApplicationStatus::Running;
                                    db.update_deployment(&deployment.id, &deployment);
                                    drop(db);
                                }
                            }
                        }
                    }
                    ApplicationStatus::DestructionRequested => {
                        let mut db = arc.lock().unwrap();
                        deployment.status = ApplicationStatus::DelegatingDestruction;
                        db.update_deployment(&deployment.id, &deployment);
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
