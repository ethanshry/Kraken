#![feature(proc_macro_hygiene, decl_macro)]

//#[macro_use]
//extern crate juniper; // = import external crate? why do I not need rocket, etc?

mod db; // looks for a file named "db.rs" and implicitly wraps it in a mod db {}
mod file_utils;
mod git_utils;
mod gitapi;
mod model;
mod rabbit;
mod routes;
mod schema;

extern crate fs_extra;
extern crate serde;

#[macro_use]
extern crate juniper;

use amiquip::{ConsumerMessage, Publish};
use bollard::image::BuildImageOptions;
use bollard::Docker;
use juniper::EmptyMutation;
use std::fs;
use std::io::prelude::*;
use std::process::Command;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;
use tar::Builder as TarBuilder;
use uuid::Uuid;

use db::{Database, ManagedDatabase};
use file_utils::{clear_tmp, copy_dir_contents_to_static, copy_dockerfile_to_dir};
use git_utils::clone_remote_branch;
use model::{
    ApplicationStatus, Orchestrator, OrchestratorInterface, Platform, Service, ServiceStatus,
};
use rabbit::{RabbitBroker, RabbitMessage, SysinfoMessage};
use schema::Query;

/// Kraken is a LAN-based deployment platform. It is designed to be highly flexible and easy to use.
/// TODO actually write this documentation

// TODO parse TOML
// https://docs.rs/toml/0.5.6/toml/

// TODO make reqwest
// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
// https://crates.io/crates/reqwest
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO get sysinfo from platform.toml
    // TODO pull static site from git and put in static folder

    clear_tmp();

    // get uuid for system, or create one
    let uuid;
    match fs::read_to_string("id.txt") {
        Ok(contents) => uuid = contents.parse::<String>().unwrap(),
        Err(_) => {
            let mut file = fs::File::create("id.txt").unwrap();
            let contents = Uuid::new_v4();
            file.write_all(contents.to_hyphenated().to_string().as_bytes())
                .unwrap();
            uuid = contents.to_hyphenated().to_string()
        }
    }

    let mut db = Database::new();

    let platform: Platform;
    // TODO handle panic here
    match fs::read_to_string("platform.json") {
        Ok(contents) => platform = serde_json::from_str(&contents).unwrap(),
        Err(_) => {
            platform = Platform::new(
                &vec![],
                &Orchestrator::new(
                    OrchestratorInterface::new(
                        Some(String::from("https://github.com/ethanshry/Kraken-UI.git")),
                        None,
                        ApplicationStatus::ERRORED,
                    )
                    .to_owned(),
                ),
                &vec![],
            );
        }
    }

    println!("Platform: {:?}", platform);

    db.update_orchestrator_interface(platform.orchestrator.ui);

    for node in platform.nodes {
        db.insert_node(node);
    }

    for deployment in platform.deployments {
        db.insert_deployment(deployment);
    }

    let db_ref = Arc::new(Mutex::new(db));

    let static_site_download_proc;
    {
        let arc = db_ref.clone();

        static_site_download_proc = std::thread::spawn(move || {
            let git_data =
                gitapi::GitApi::get_tail_commits_for_repo_branches("ethanshry", "kraken-ui");

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

    // Thread to post sysinfo every 5 seconds
    let system_status_proc;
    {
        // TODO do I need this???
        let arc = db_ref.clone();

        system_status_proc = std::thread::spawn(move || {
            let broker;

            match RabbitBroker::new("amqp://localhost:5672") {
                Some(b) => broker = b,
                None => {
                    // TODO spin up service and try again
                    broker = RabbitBroker::new("amqp://localhost:5672").unwrap()
                }
            }

            // closure for auto unlock
            || -> () {
                let mut db = arc.lock().unwrap();
                db.add_platform_service(Service::new(
                    "rabbitmq",
                    "0.1.0",
                    "amqp://localhost:5672",
                    ServiceStatus::OK,
                ))
                .unwrap();
            }();

            let publisher = broker.get_publisher();
            loop {
                std::thread::sleep(std::time::Duration::new(5, 0));
                let system = sysinfo::System::new_all();
                let msg = format!(
                    "{}|{}|{}|{}",
                    system.get_free_memory(),
                    system.get_used_memory(),
                    system.get_uptime(),
                    system.get_load_average().five,
                );
                let msg = SysinfoMessage::build_message(&uuid, msg);
                publisher
                    .publish(Publish::new(&msg[..], "sysinfo"))
                    .unwrap();
            }
        });
    }

    // Thread to consume sysinfo queue
    let rabbit_consumer_proc;
    {
        let arc = db_ref.clone();
        let broker;

        match RabbitBroker::new("amqp://localhost:5672") {
            Some(b) => broker = b,
            None => {
                // TODO spin up service and try again
                broker = RabbitBroker::new("amqp://localhost:5672").unwrap()
            }
        }
        rabbit_consumer_proc = std::thread::spawn(move || {
            let consumer = broker.get_consumer("sysinfo");

            for (_, message) in consumer.receiver().iter().enumerate() {
                match message {
                    ConsumerMessage::Delivery(delivery) => {
                        let data = SysinfoMessage::deconstruct_message(&delivery.body);
                        println!("Recieved Message on the Sysinfo channel");

                        // TODO clean up all this unsafe unwrapping
                        let mut db = arc.lock().unwrap();
                        if let Some(n) = db.get_node(data.get(0).unwrap()) {
                            let mut new_node = n.clone();
                            new_node.set_id(data.get(0).unwrap());
                            new_node.update(
                                data.get(1).unwrap(),
                                data.get(2).unwrap(),
                                data.get(3).unwrap(),
                                data.get(4).unwrap(),
                            );
                            db.insert_node(new_node);
                        } else {
                            db.insert_node(model::Node::from_msg(&data));
                        }

                        consumer.ack(delivery).expect("noack");
                    }
                    other => {
                        println!("Consumer ended: {:?}", other);
                        break;
                    }
                }
            }
        });
    }

    // deploy image

    let res = std::thread::spawn(move || {
        let tmp_dir_path = "tmp/process";

        let docker: Docker = Docker::connect_with_unix_defaults().unwrap();
        let container_guid = Uuid::new_v4().to_hyphenated().to_string();

        println!("Creating Container {}", &container_guid);

        println!("Cloning docker process...");
        clone_remote_branch(
            "https://github.com/ethanshry/scapegoat.git",
            "master",
            tmp_dir_path,
        )
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

        let res = Command::new("docker")
            .current_dir("tmp/process")
            .arg("build")
            .arg(".")
            .arg("-t")
            .arg(&container_guid).spawn().expect("error in docker build");

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

    // launch the API server
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

    static_site_download_proc.join().unwrap();
    system_status_proc.join().unwrap();
    rabbit_consumer_proc.join().unwrap();
    Ok(())
}
