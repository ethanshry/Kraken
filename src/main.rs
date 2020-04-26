#![feature(proc_macro_hygiene, decl_macro)]

//#[macro_use]
//extern crate juniper; // = import external crate? why do I not need rocket, etc?

mod db; // looks for a file named "db.rs" and implicitly wraps it in a mod db {}
mod model;
mod rabbit;
mod routes;
mod schema;

extern crate fs_extra;

use amiquip::{ConsumerMessage, Publish};
use fs_extra::{copy_items, dir::copy};
use git2::Repository;
use juniper::EmptyMutation;
use std::fs;
use std::io::prelude::*;
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use sysinfo::SystemExt;
use uuid::Uuid;

use db::{Database, ManagedDatabase};
use rabbit::{RabbitBroker, RabbitMessage, SysinfoMessage};
use schema::Query;

// TODO parse TOML
// https://docs.rs/toml/0.5.6/toml/

// TODO make reqwest
// https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
// https://crates.io/crates/reqwest

fn copy_dir_contents_to_static(dir: &str) -> () {
    fn copy_dir_with_parent(root: &str, dir: &str) -> () {
        if root != "" {
            fs::create_dir(format!("static/{}", root));
        }
        for item in fs::read_dir(dir).unwrap() {
            let path = &item.unwrap().path();
            if path.is_dir() {
                let folder_name = path.to_str().unwrap().split("/").last().unwrap();
                copy_dir_with_parent(
                    format!("{}/{}", root, folder_name).as_str(),
                    path.to_str().unwrap(),
                );
            } else {
                copy_file_to_static(root, path.to_str().unwrap());
            }
        }
    }

    copy_dir_with_parent("", dir)
}

fn copy_file_to_static(target_subdir: &str, file_path: &str) -> () {
    let item_name = file_path.split("/").last().unwrap();
    match target_subdir {
        "" => fs::copy(file_path, format!("static/{}", item_name)),
        _ => fs::copy(file_path, format!("static/{}/{}", target_subdir, item_name)),
    };
}

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

    let db_ref = Arc::new(Mutex::new(db));

    let static_site_download_proc = std::thread::spawn(|| {
        /*
        let repo = match Repository::clone("https://github.com/ethanshry/kraken-ui", "tmp/site") {
            Ok(r) => r,
            Err(e) => panic!("Failed to Clone: {}", e),
        };
        */
        //let mut reference = repo.find_reference("refs/remotes/origin/build").unwrap();
        //reference.set_target(fetch_commit.id(), "Fast-Forward")?;
        //repo.set_head("refs/heads/build").unwrap();

        //repo.set_head("build");
        /*
        match repo.checkout_head(None) {
            Ok(_) => println!("OK"),
            Err(e) => println!("Err: {}", e),
        };
        */

        /*
                repo.find_remote("origin")
                    .unwrap()
                    .fetch(&["build"], None, None);
        */

        let mut cmd = std::process::Command::new("git")
            .arg("clone")
            .arg("-b")
            .arg("build")
            .arg("https://github.com/ethanshry/Kraken-UI.git")
            .arg("tmp/site")
            .spawn()
            .expect("err in clone");

        cmd.wait();

        println!("Directory succesfully cloned!");

        let options = fs_extra::dir::CopyOptions::new();

        copy_dir_contents_to_static("tmp/site/build");

        //copy("tmp/site/build/", "static", &options).unwrap();

        fs::remove_dir_all("tmp/site").unwrap();
    });

    // Thread to post sysinfo every 5 seconds
    let system_status_proc;
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
