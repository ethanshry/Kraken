#![feature(proc_macro_hygiene, decl_macro)]

//#[macro_use]
//extern crate rocket;
//extern crate juniper; // = import external crate? why do I not need rocket, etc?
mod docker;
mod rabbit;

mod db; // looks for a file named "db.rs" and implicitly wraps it in a mod db {}
mod file_utils;
mod git_utils;
mod gitapi;
mod model;
mod routes;
mod schema;
mod utils;
mod worker;

extern crate fs_extra;
extern crate serde;
extern crate strum;

#[macro_use]
extern crate juniper;
extern crate strum_macros;

use db::{Database, ManagedDatabase};
use dotenv;
use file_utils::{clear_tmp, copy_dir_contents_to_static, copy_dockerfile_to_dir};
use futures_util::stream::StreamExt;
use git_utils::clone_remote_branch;
use juniper::EmptyMutation;
use log::info;
use model::{ApplicationStatus, Platform, Service, ServiceStatus};
use rabbit::{sysinfo_message::SysinfoMessage, QueueLabel, RabbitBroker, RabbitMessage};
use schema::Query;
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;
use uuid::Uuid;

/// Kraken is a LAN-based deployment platform. It is designed to be highly flexible and easy to use.
/// TODO actually write this documentation

// TODO parse TOML
// https://docs.rs/toml/0.5.6/toml/

// TODO make reqwest
// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
// https://crates.io/crates/reqwest

// TODO build system_uuid into rabbit connection as a middlelayer before send

#[derive(Debug, Clone)]
pub enum NodeMode {
    WORKER,
    ORCHESTRATOR,
}

// TODO implement
fn get_node_mode() -> NodeMode {
    match std::env::var("NODE_MODE")
        .unwrap_or(String::from("WORKER"))
        .as_str()
    {
        "ORCHESTRATOR" => NodeMode::ORCHESTRATOR,
        _ => NodeMode::WORKER,
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    dotenv::dotenv().ok();
    env_logger::init();
    let addr: String =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672".into());

    // TODO pull static site from git and put in static folder

    // clear all tmp files
    clear_tmp();

    // get uuid for system, or create one
    let system_uuid = utils::get_system_id();
    // TODO How should I handle this type of thing?
    std::env::set_var("SYSID", system_uuid);

    // determine if should be an orchestrator or a worker
    let mode = get_node_mode();

    let broker: RabbitBroker = match mode {
        // Connect directly to rabbit
        NodeMode::WORKER => match RabbitBroker::new(&addr).await {
            Some(b) => b,
            None => panic!("Could not establish rabbit connection"),
        },
        // Deploy rabbit then connect
        NodeMode::ORCHESTRATOR => {
            // TODO Deploy rabbit
            let broker = match RabbitBroker::new(&addr).await {
                Some(b) => b,
                None => panic!("Could not establish rabbit connection"),
            };

            // deploy queues

            broker.declare_queue(QueueLabel::Sysinfo.as_str()).await;
            broker.declare_queue(QueueLabel::Deployment.as_str()).await;

            broker
        }
    };

    match mode {
        NodeMode::WORKER => {
            // post system status

            // deploy docker services if requested
        }
        NodeMode::ORCHESTRATOR => {
            // establish db connection
            let mut db = Database::new();

            let platform: Platform = utils::load_or_create_platform(&mut db);

            println!("Platform: {:?}", platform);

            let db_ref = Arc::new(Mutex::new(db));

            // download site
            let static_site_download_proc;
            {
                let arc = db_ref.clone();

                static_site_download_proc = tokio::spawn(async move {
                    let git_data = gitapi::GitApi::get_tail_commits_for_repo_branches(
                        "ethanshry",
                        "kraken-ui",
                    );

                    let mut sha = String::from("");

                    let should_update_ui = match git_data {
                        Some(data) => {
                            let mut flag = true;
                            for branch in data {
                                if branch.name == "build" {
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
                        println!("No build branch found, cannot update kraken-ui");
                        return;
                    }

                    if should_update_ui {
                        println!("Cloning updated UI...");
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

                    println!("Kraken-UI is now available at commit SHA: {}", sha);
                });
            }

            // setup dns records

            // consume system log queues

            // consumer sysinfo queue
            let sysinfo_consumer = {
                let arc = db_ref.clone();
                let system_uuid = system_uuid.clone();
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
                        let new_node = model::Node::from_incomplete(
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
                    broker
                        .consume_queue(&system_uuid, QueueLabel::Sysinfo.as_str(), &handler)
                        .await;
                })
            };

            // consume deployment status queue
            let deployment_consumer = {
                let arc = db_ref.clone();

                let system_uuid = system_uuid.clone();

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
                    return ();
                };

                tokio::spawn(async move {
                    broker
                        .consume_queue(&system_uuid, QueueLabel::Sysinfo.as_str(), &handler)
                        .await;
                })
            };
            // work sender queue

            // launch rocket
            let server = tokio::spawn(async move {
                static_site_download_proc.await;
                rocket::ignite()
                    .manage(ManagedDatabase::new(db_ref.clone()))
                    .manage(routes::Schema::new(Query, EmptyMutation::<Database>::new()))
                    .mount(
                        "/",
                        rocket::routes![
                            routes::root,
                            routes::graphiql,
                            routes::get_graphql_handler,
                            routes::post_graphql_handler,
                            routes::site
                        ],
                    )
                    .launch();
            });

            // monitor git

            // deploy docker services if determined to be best practice
        }
    };

    // Thread to post sysinfo every 5 seconds
    let system_status_proc;
    {
        let arc = db_ref.clone();
        let producer = broker.get_channel().await;
        let system_uuid = system_uuid.clone();
        let addr = addr.clone();

        let publisher = broker.get_channel().await;

        system_status_proc = tokio::spawn(async move {
            // closure for auto unlock
            || -> () {
                let mut db = arc.lock().unwrap();
                db.add_platform_service(Service::new(
                    "rabbitmq",
                    "0.1.0",
                    &addr,
                    ServiceStatus::OK,
                ))
                .unwrap();
            }();

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
        });
    }

    // deploy image

    let res = std::thread::spawn(move || {
        let container_guid = Uuid::new_v4().to_hyphenated().to_string();
        let uri = "https://github.com/ethanshry/scapegoat.git";

        let broker;

        match RabbitBroker::new("amqp://localhost:5672") {
            Some(b) => broker = b,
            None => {
                // TODO spin up service and try again
                broker = RabbitBroker::new("amqp://localhost:5672").unwrap()
            }
        }

        let publisher = broker.get_publisher();

        let mut msg = DeploymentMessage::new(&container_guid);

        publisher
            .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
            .unwrap();

        // TODO make function to execute a thing in a tmp dir which auto-cleans itself
        let tmp_dir_path = "tmp/process";

        println!("Creating Container {}", &container_guid);

        msg.update_message(
            ApplicationStatus::RETRIEVING,
            &format!("Retrieving Application Data from {}", uri),
        );

        publisher
            .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
            .unwrap();

        println!("Cloning docker process...");
        clone_remote_branch(uri, "master", tmp_dir_path)
            .wait()
            .unwrap();

        // Inject the proper dockerfile into the project
        // TODO read this from the project's toml file
        copy_dockerfile_to_dir("python36.dockerfile", tmp_dir_path);

        /*
        // tar the github repo
        let mut ball =
            TarBuilder::new(fs::File::create(format!("tmp/tar/{}.tar", &container_guid)).unwrap());
        //let mut ball = TarBuilder::new(Vec::new());

        ball.append_dir_all(tmp_dir_path, ".").unwrap();

        ball.finish();

        let options = BuildImageOptions {
            t: container_guid.clone(),
            rm: true,
            ..Default::default()
        };

        */

        println!("module path is {}", module_path!());

        msg.update_message(ApplicationStatus::BUILDING, "");

        publisher
            .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
            .unwrap();

        let res = Command::new("docker")
            .current_dir("tmp/process")
            .arg("build")
            .arg(".")
            .arg("-t")
            .arg(&container_guid)
            .output()
            .expect("error in docker build");

        println!("docker build status: {}", res.status);

        // TODO write all build info to build log

        if res.stderr.len() != 0 {
            println!("Error in build");
            msg.update_message(ApplicationStatus::ERRORED, "Error in build process");

            publisher
                .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
                .unwrap();
        } else {
            msg.update_message(ApplicationStatus::DEPLOYING, "");

            publisher
                .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
                .unwrap();

            let res = Command::new("docker")
                .arg("run")
                .arg("-i")
                .arg("-p")
                .arg("9000:9000")
                .arg("--name")
                .arg(&container_guid)
                .arg("-d")
                .arg("-t")
                .arg(&container_guid)
                .output()
                .expect("error in docker run");

            // TODO write all deploy info to deploy log

            println!("docker run status: {}", res.status);

            if res.stderr.len() != 0 {
                println!("Error in deploy");
                msg.update_message(ApplicationStatus::ERRORED, "Error in deployment");

                publisher
                    .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
                    .unwrap();
            } else {
                println!(
                    "Docker container id: {}",
                    &String::from_utf8_lossy(&res.stdout)
                );

                msg.update_message(ApplicationStatus::DEPLOYED, "");

                publisher
                    .publish(Publish::new(&msg.build_message(&system_uuid), "deployment"))
                    .unwrap();

                println!(
                    "Application instance {} deployed succesfully",
                    &container_guid
                );
            }
        }

        // write stderr to stdout
        //io::stderr().write_all(&res.stdout).unwrap();

        //let mut ball = fs::File::open(format!("/tmp/tar/{}", &container_guid)).unwrap();

        //let mut contents = Vec::new();
        //ball.read_to_end(&mut contents).unwrap();

        //let res = docker.build_image(options, None, Some(contents.into()));
        /*.map(|v| {
            println!("{:?}", v);
            v
        })
        .map_err(|e| {
            println!("{:?}", e);
            e
        })
        .collect::<Vec<Result<BuildImageResults, bollard::errors::Error>>>()
        .await;
        */
        //res.try_collect().await;
    });

    // let uri = "https://github.com/ethanshry/scapegoat.git";
    system_status_proc.await;
    Ok(())
}
