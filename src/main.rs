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
use juniper::EmptyMutation;
use std::collections::HashMap;
use std::fs;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;
use uuid::Uuid;

use db::{Database, ManagedDatabase};
use file_utils::{copy_dir_contents_to_static, copy_file_to_static};
use git_utils::clone_remote_branch;
use model::{ApplicationStatus, Orchestrator, OrchestratorInterface, Platform};
use rabbit::{RabbitBroker, RabbitMessage, SysinfoMessage};
use schema::Query;

/// Kraken is a LAN-based deployment platform. It is designed to be highly flexible and easy to use.
/// TODO actually write this documentation

// TODO parse TOML
// https://docs.rs/toml/0.5.6/toml/

// TODO make reqwest
// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
// https://crates.io/crates/reqwest

fn main() {
    // TODO get sysinfo from platform.toml
    // TODO pull static site from git and put in static folder

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

    let db = Database::new();

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

    let db_ref = Arc::new(Mutex::new(db));

    let static_site_download_proc;
    {
        let arc = db_ref.clone();

        static_site_download_proc = std::thread::spawn(move || {
            let git_data =
                gitapi::GitApi::get_tail_commits_for_repo_branches("ethanshry", "kraken");

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

            println!("Kraken-UI is now available at commit SHA:{}", sha);
            //let db = arc.lock().unwrap();
            //db.update_orchestrator_interface(OrchestratorInterface::new());
        });
    }

    // Thread to post sysinfo every 5 seconds
    let system_status_proc;
    {
        // TODO do I need this???
        let arc = db_ref.clone();
        let broker;

        match RabbitBroker::new("amqp://localhost:5672") {
            Some(b) => broker = b,
            None => {
                // TODO spin up service and try again
                broker = RabbitBroker::new("amqp://localhost:5672").unwrap()
            }
        }
        system_status_proc = std::thread::spawn(move || {
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
                // TODO do I need this???
                let _ = arc.lock().unwrap();
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
                            new_node.update(
                                data.get(1).unwrap(),
                                data.get(2).unwrap(),
                                data.get(3).unwrap(),
                                data.get(4).unwrap(),
                            );
                            db.insert_node(data.get(0).unwrap(), new_node);
                        } else {
                            db.insert_node(data.get(0).unwrap(), model::Node::from_msg(&data));
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
}
