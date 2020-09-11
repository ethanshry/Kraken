use crate::db::{Database, ManagedDatabase};
use crate::file_utils::{clear_tmp, copy_dir_contents_to_static};
use crate::git_utils::clone_remote_branch;
use crate::model::{ApplicationStatus, Service, ServiceStatus};
use crate::platform_executor::{GenericNode, SetupFaliure, Task, TaskFaliure};
use crate::rabbit::{
    deployment_message::DeploymentMessage,
    sysinfo_message::SysinfoMessage,
    work_request_message::{WorkRequestMessage, WorkRequestType},
    QueueLabel, RabbitMessage,
};
use crate::schema::Query;
use juniper::EmptyMutation;
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
            .manage(crate::routes::Schema::new(
                Query,
                EmptyMutation::<Database>::new(),
            ))
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
        copy_dir_contents_to_static("tmp/site/build");
        fs::remove_dir_all("tmp/site").unwrap();
    }

    info!("Kraken-UI is now available at commit SHA: {}", sha);
}

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

    // clear all tmp files
    clear_tmp();

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

    // setup dns records

    // consume system log queues

    // consumer sysinfo queue
    let sysinfo_consumer = {
        let arc = o.db_ref.clone();
        let system_uuid = node.system_id.clone();
        let addr = node.rabbit_addr.clone();
        let handler = move |data: Vec<u8>| {
            let (node, message) = SysinfoMessage::deconstruct_message(&data);
            info!("Recieved Message on the Sysinfo channel");
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
            info!("Recieved Message on the Deployment channel");

            info!(
                "{} : Deployment {}\n\t{} : {}",
                node,
                message.deployment_id,
                message.deployment_status,
                message.deployment_status_description
            );

            // TODO add info to the db

            return ();
        };

        tokio::spawn(async move {
            let broker = match crate::platform_executor::connect_to_rabbit_instance(&addr).await {
                Ok(b) => b,
                Err(e) => panic!("Could not establish rabbit connection"),
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

    // work sender queue

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
                if deployment.status == ApplicationStatus::REQUESTED {
                    // TODO Validate deployment here?
                    // Look for free nodes to distribute tasks to
                    deployment.status = ApplicationStatus::INITIALIZED;
                    let mut db = arc.lock().unwrap();
                    db.update_deployment(&deployment.id, &deployment);
                    let nodes = db.get_nodes();
                    match nodes {
                        None => {
                            deployment.status = ApplicationStatus::ERRORED;
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

                            // curr_node will recieve the work

                            // Send work to curr_node
                            let msg = WorkRequestMessage::new(
                                WorkRequestType::RequestDeployment,
                                Some(&deployment.id),
                                Some(&deployment.src_url),
                                None,
                            );
                            let publisher = node.broker.as_ref().unwrap().get_channel().await;
                            msg.send(&publisher, &node.system_id).await;

                            // Deployment has been scheduled
                        }
                    }
                }
            }
        }

        // TODO look for deployments which need to be updated

        // TODO look for deployments which need to be cancelled
    }()
    .await;
    Ok(())
}
