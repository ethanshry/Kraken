use crate::db::{Database, ManagedDatabase};
use crate::file_utils::{clear_tmp, copy_dir_contents_to_static};
use crate::git_utils::clone_remote_branch;
use crate::model::{Platform, Service, ServiceStatus};
use crate::platform_executor::{
    DeploymentTask, ExecutionNode, GenericNode, NodeUtils, QueueTask, SetupFaliure, TaskFaliure,
};
use crate::rabbit::{
    deployment_message::DeploymentMessage, sysinfo_message::SysinfoMessage, QueueLabel,
    RabbitBroker, RabbitMessage,
};
use crate::schema::Query;
use async_trait::async_trait;
use dotenv;
use juniper::EmptyMutation;
use log::{info, warn};
use std::fs;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;

pub struct Orchestrator {
    api_server: (),
    database: Option<Database>,
    db_ref: Arc<Mutex<Database>>,
    node_utils: GenericNode,
}

// TODO add broker to utils

fn create_api_server(o: &'static Orchestrator) -> tokio::task::JoinHandle<()> {
    let options = rocket_cors::CorsOptions {
        ..Default::default()
    }
    .to_cors()
    .unwrap();

    // launch rocket
    let server = tokio::spawn(async move {
        rocket::ignite()
            .manage(ManagedDatabase::new(o.db_ref.clone()))
            .manage(crate::routes::Schema::new(
                Query,
                EmptyMutation::<Database>::new(),
            ))
            .mount(
                "/",
                rocket::routes![
                    crate::routes::root,
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

impl Orchestrator {
    /// Creates a new Orchestrator Object
    pub fn new(system_id: &str, rabbit_addr: &str) -> Orchestrator {
        let db = Database::new();
        Orchestrator {
            api_server: (),
            database: Some(db),
            db_ref: Arc::new(Mutex::new(db)),
            node_utils: GenericNode::new(system_id, rabbit_addr),
        }
    }
    /*
    async fn fetch_ui(&self) -> () {
        let git_data =
            crate::gitapi::GitApi::get_tail_commits_for_repo_branches("ethanshry", "kraken-ui")
                .await;

        let mut sha = String::from("");

        let should_update_ui = match git_data {
            Some(data) => {
                let mut flag = true;
                for branch in data {
                    if branch.name == "build" {
                        let arc = self.db_ref.clone();
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
    */

    pub fn deploy_rabbit_instance(&self) {}
}

#[async_trait]
impl ExecutionNode for Orchestrator {
    async fn setup(&self) -> Result<(), SetupFaliure> {
        // clear all tmp files
        clear_tmp();

        // Establish RabbitMQ
        self.deploy_rabbit_instance();

        match self.node_utils.connect_to_rabbit_instance().await {
            Ok(b) => self.node_utils.broker = Some(b),
            Err(e) => {
                warn!("{}", e);
                return Err(SetupFaliure::NoRabbit);
            }
        }

        // Unwrap is safe here because we will have returned Err if broker is None
        self.node_utils
            .broker
            .unwrap()
            .declare_queue(QueueLabel::Sysinfo.as_str())
            .await;

        self.node_utils
            .broker
            .unwrap()
            .declare_queue(QueueLabel::Deployment.as_str())
            .await;

        // TODO what is this for?
        // todo move into this struct
        let platform: Platform = crate::utils::load_or_create_platform(&mut self.database.unwrap());

        info!("Platform: {:?}", platform);

        // closure for auto unlock
        || -> () {
            let arc = self.db_ref.clone();
            let mut db = arc.lock().unwrap();
            db.add_platform_service(Service::new(
                "rabbitmq",
                "0.1.0",
                &self.node_utils.rabbit_addr,
                ServiceStatus::OK,
            ))
            .unwrap();
        }();

        // download site
        //self.fetch_ui().await;

        self.node_utils.queue_consumers = Some(vec![]);

        // setup dns records

        // consume system log queues

        // consumer sysinfo queue
        let sysinfo_consumer = {
            let arc = self.db_ref.clone();
            let system_uuid = self.node_utils.system_id.clone();
            let addr = self.node_utils.rabbit_addr.clone();
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
                let broker = match self.node_utils.connect_to_rabbit_instance().await {
                    Ok(b) => b,
                    Err(e) => panic!("Could not establish rabbit connection"),
                };
                broker
                    .consume_queue(&system_uuid, QueueLabel::Sysinfo.as_str(), &handler)
                    .await;
            })
        };

        /*
        self.node_utils.queue_consumers.unwrap().push(QueueTask {
            task: sysinfo_consumer,
            queue_label: QueueLabel::Sysinfo.as_str(),
        });
        */
        // consume deployment status queue
        let deployment_consumer = {
            let arc = self.db_ref.clone();

            let system_uuid = self.node_utils.system_id.clone();
            let addr = self.node_utils.rabbit_addr.clone();
            let handler = move |data: Vec<u8>| {
                let (node, message) = SysinfoMessage::deconstruct_message(&data);
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
                let broker = match self.node_utils.connect_to_rabbit_instance().await {
                    Ok(b) => b,
                    Err(e) => panic!("Could not establish rabbit connection"),
                };
                broker
                    .consume_queue(&system_uuid, QueueLabel::Deployment.as_str(), &handler)
                    .await;
            })
        };
        /*
        self.node_utils.queue_consumers.unwrap().push(QueueTask {
            task: deployment_consumer,
            queue_label: QueueLabel::Deployment.as_str(),
        });
        */
        // work sender queue

        // You can also deserialize this

        // monitor git

        // deploy docker services if determined to be best practice
        Ok(())
    }

    async fn execute(&self) -> Result<(), TaskFaliure> {
        // Thread to post sysinfo every 5 seconds
        /*
        let system_status_proc = {
            let system_uuid = self.node_utils.system_id.clone();

            let publisher = self.node_utils.broker.unwrap().get_channel().await;

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

        system_status_proc.await;
        */
        Ok(())
    }
}
