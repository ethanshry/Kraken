#![feature(proc_macro_hygiene, decl_macro)]

//#[macro_use]
//extern crate juniper; // = import external crate? why do I not need rocket, etc?

mod db; // looks for a file named "db.rs" and implicitly wraps it in a mod db {}
mod model;
mod routes;
mod schema;

use amiquip::{
    Connection, ConsumerMessage, ConsumerOptions, Exchange, Publish, QueueDeclareOptions, Result,
};
use juniper::EmptyMutation;
use std::fs;
use std::io::prelude::*;
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use sysinfo::SystemExt;
use uuid::Uuid;

use db::{Database, ManagedDatabase};
use schema::Query;

/// TODO refactor into connection and broker
struct RabbitBroker {
    /// Connection to the Rabbit Instance (Should be one per device)
    conn: Connection,
    /// Multiplexed access to the conn, should be one per thread
    channel: amiquip::Channel,
}

impl RabbitBroker {
    /// Creates a new RabbitBroker, returns None if no rabbit connection could be made
    /// This indicates that the machine needs to spin up a rabbit instance
    pub fn new(addr: &str) -> Option<RabbitBroker> {
        let conn = Connection::insecure_open(addr);
        match conn {
            Ok(v) => {
                let mut conn = v;
                let channel = conn.open_channel(None).expect("no channel");

                Some(RabbitBroker {
                    conn: conn,
                    channel: channel,
                })
            }
            Err(_) => None,
        }
    }

    /// Gets a new publisher for the existing broker channel. This should happen on the thread it is operating on
    pub fn get_publisher(&self) -> Exchange {
        Exchange::direct(&self.channel)
    }

    pub fn get_consumer(&self, queue_label: &str) -> amiquip::Consumer {
        self.channel
            .queue_declare(queue_label, QueueDeclareOptions::default())
            .expect("Could not establish consumer queue")
            .consume(ConsumerOptions::default())
            .expect("Could not consume the queue")
    }

    /*
    pub fn close(&self) -> Result<()> {
        self.conn.close()
    }
    */
}

trait RabbitMessage {
    fn build_message(identifier: &String, packet_data: String) -> Vec<u8>;
    fn deconstruct_message(packet_data: &Vec<u8>) -> Vec<String>;
}

struct SysinfoMessage {}

impl RabbitMessage for SysinfoMessage {
    fn build_message(identifier: &String, packet_data: String) -> Vec<u8> {
        format!("{}|{}", identifier, packet_data)
            .as_bytes()
            .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> Vec<String> {
        Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split("|")
                .map(|s| s.to_string()),
        )
    }
}

/*
impl Future for RabbitBroker {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.conn.poll(&mut cx).map(|_| ())
    }
}
*/

fn main() {
    /*
    // Open connection.
    let mut connection =
        Connection::insecure_open("amqp://localhost:5672").expect("couldn't get connection");

    // Open a channel - None says let the library choose the channel ID.
    let channel = connection.open_channel(None).expect("rip");

    // Get a handle to the direct exchange on our channel.
    let exchange = Exchange::direct(&channel);

    // Publish a message to the "hello" queue.
    exchange
        .publish(Publish::new("hello there".as_bytes(), "hello"))
        .expect("couldn't pub");

    // Declare the "hello" queue.
    let queue = channel
        .queue_declare("hello", QueueDeclareOptions::default())
        .expect("noqueue");

    // Start a consumer.
    let consumer = queue
        .consume(ConsumerOptions::default())
        .expect("noconsume");
    println!("Waiting for messages. Press Ctrl-C to exit.");

    for (i, message) in consumer.receiver().iter().enumerate() {
        match message {
            ConsumerMessage::Delivery(delivery) => {
                let body = String::from_utf8_lossy(&delivery.body);
                println!("({:>3}) Received [{}]", i, body);
                consumer.ack(delivery).expect("noack");
            }
            other => {
                println!("Consumer ended: {:?}", other);
                break;
            }
        }
    }
    */

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

    let broker;

    match RabbitBroker::new("amqp://localhost:5672") {
        Some(b) => broker = b,
        None => {
            // TODO spin up service and try again
            broker = RabbitBroker::new("amqp://localhost:5672").unwrap()
        }
    }

    //let broker = Arc::new(Mutex::new(broker));

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
            // Declare the "hello" queue.
            let consumer = broker.get_consumer("sysinfo");

            for (i, message) in consumer.receiver().iter().enumerate() {
                match message {
                    ConsumerMessage::Delivery(delivery) => {
                        let data = SysinfoMessage::deconstruct_message(&delivery.body);
                        println!("({:>3}) Received [{}]", i, data.get(0).unwrap());

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

    rocket::ignite()
        .manage(ManagedDatabase::new(db_ref.clone()))
        .manage(routes::Schema::new(Query, EmptyMutation::<Database>::new()))
        .mount(
            "/",
            rocket::routes![
                routes::root,
                routes::graphiql,
                routes::get_graphql_handler,
                routes::post_graphql_handler
            ],
        )
        .launch();

    system_status_proc.join().unwrap();
    rabbit_consumer_proc.join().unwrap();

    //connection.close().expect("err on close");

    //let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    //let conn = Connection::connect(&addr, ConnectionProperties::default().with_tokio()).await?;

    //let channel_a = conn.create_channel().await?;
    //let channel_b = conn.create_channel().await?;
    /*
    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
        */
    /*let db = Database::new();

    let db_ref = Arc::new(Mutex::new(db));
        let publisher = broker
            .get_publisher("system-status")
            .await
            .expect("cannot get publisher");
    */
    /*
    let consumer = channel_b
        .clone()
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
        */

    /*
    consumer.set_delegate(move |delivery: DeliveryResult| {
        let channel_b = channel_b.clone();
        async move {
            let delivery = delivery.expect("error caught in in consumer");
            if let Some(delivery) = delivery {
                channel_b
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .expect("failed to ack");

                print!(
                    "{} : {}",
                    delivery.delivery_tag,
                    std::str::from_utf8(&delivery.data).unwrap()
                );
            }
        }
    });
    */
    /*
        let payload = b"Hello world!";
    let db = Database::new();

        let db_ref = Arc::new(Mutex::new(db));erties::default(),
                )
                .await?
                .await?;
            assert_eq!(confirm, Confirmation::NotRequested);
        }
        */
}

/*
#[tokio::main]
fn main() -> Result<()> {
    let db = Database::new();

    let db_ref = Arc::new(Mutex::new(db));

    /*
        RABBIT MQ
    */

    //let mut executor = tokio_amqp::LocalPool::new();
    //let spawner = executor.spawner();
    let addr = "amqp://127.0.0.1:5672";
    let conn = Connection::connect(&addr, ConnectionProperties::default().with_tokio()).await?;

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let consumer = channel_b
        .clone()
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    consumer.set_delegate(move |delivery: DeliveryResult| {
        let channel_b = channel_b.clone();
        async move {
            let delivery = delivery.expect("error caught in in consumer");
            if let Some(delivery) = delivery {
                channel_b
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .expect("failed to ack");
            }
        }
    });

    let payload = b"Hello world!";

    loop {
        let confirm = channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
    };
    /*
    loop {}
    std::thread::sleep(std::time::Duration::new(5, 0));
    let mut system = sysinfo::System::new();
    system.refresh_all();
    println!("total memory: {} kB", system.get_total_memory());
    println!("used memory : {} kB", system.get_used_memory());
    let mut db = arc.lock().unwrap();
    if let Some(n) = db.get_node("test") {
        let mut new_node = n.clone();
        new_node.set_disk_ram_cpu(12.0, 12, 12);
        db.insert_node("test", new_node);
    }
    */let res = tokio::spawn(async move || {

        let addr = "amqp://127.0.0.1:5672";

        let conn = Connection::connect(&addr, ConnectionProperties::default().with_tokio()).await?;

        let channel_a = conn.create_channel().await?;
        let channel_b = conn.create_channel().await?;
        let queue = channel_a
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        let consumer = channel_b
            .clone()
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
            let _consumer = tokio::spawn(async move {
                consumer
                    .for_each(move |delivery| {
                        let delivery = delivery.expect("error caught in in consumer");
                        channel_b
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .map(|_| ())
                    })
                    .awaitResult<(), Box<dyn std::error::Error>>
            });
            let payload = b"Hello world!";
            loop {
                let confirm = channel_a
                    .basic_publish(
                        "",
                        "hello",
                        BasicPublishOptions::default(),
                        payload.to_vec(),
                        BasicProperties::default(),
                    )
                    .await?
                    .await?;
                assert_eq!(confirm, Confirmation::NotRequested);
            }
    });
    let res = executor.run_until(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

        let channel_a = conn.create_channel().await?;
        let channel_b = conn.create_channel().await?;

        let queue = channel_a
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let consumer = channel_b
            .clone()
            .bas
    info!("Declared queue {:?}", queue);it?;
        let _consumer = spawner.spawn_local(async move {
            consumer
                .for_each(move |delivery| {
                    let delivery = delivery.expect("error caught in in consumer");
                    channel_b
                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                        .map(|_| ())
                })
                .awaitResult<(), Box<dyn std::error::Error>>
        });

        let payload = b"Hello world!";

        loop {
            let confirm = channel_a
                .basic_publish(
                    "",
                    "hello",
               # futures-executor = "^0.3.0"
# futures-util = "^0.3.0"     BasicPublishOptions::default(),
                    payload.to_vec(),
                    BasicProperties::default(),
                )
                .await?
                .await?;
            assert_eq!(confirm, Confirmation::NotRequested);
        }
    });
    */

/*

*/
/*
    let child_proc;
    {
        let arc = db_ref.clone();
        child_proc = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::new(5, 0));
            let mut system = sysinfo::System::new();
            system.refresh_all();
            println!("total memory: {} kB", system.get_total_memory());
            println!("used memory : {} kB", system.get_used_memory());
            let mut db = arc.lock().unwrap();
            if let Some(n) = db.get_node("test") {
                let mut new_node = n.clone();
                new_node.set_disk_ram_cpu(12.0, 12, 12);
                db.insert_node("test", new_node);
            }
        });
    }

    rocket::ignite()
        .manage(ManagedDatabase::new(db_ref.clone()))
        .manage(routes::Schema::new(Query, EmptyMutation::<Database>::new()))
        .mount(
            "/",
            rocket::routes![
                routes::root,
                routes::graphiql,
                routes::get_graphql_handler,
                routes::post_graphql_handler
            ],
        )
        .launch();

    child_proc.joi# futures-executor = "^0.3.0"
# futures-util = "^0.3.0"
/*
use futures_executor::LocalPool;
use futures_util::{future::FutureExt, stream::StreamExt, task::LocalSpawnExt};
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;

fn main() -> Result<()> {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let mut executor = LocalPool::new();
    let spawner = executor.spawner();

    executor.run_until(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_default_executor(8),
        )
        .await?;

        info!("CONNECTED");

        let channel_a = conn.create_channel().await?;
        let channel_b = conn.create_channel().await?;

        let queue = channel_a
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        info!("Declared queue {:?}", queue);

        let consumer = channel_b
            .clone()
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        let _consumer = spawner.spawn_local(async move {
            info!("will consume");
            consumer
                .for_each(move |delivery| {
                    let delivery = delivery.expect("error caught in in consumer");
                    channel_b
                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                        .map(|_| ())
                })
                .await
        });

        let payload = b"Hello world!";

        loop {
            let confirm = channel_a
                .basic_publish(
                    "",
                    "hello",
                    BasicPublishOptions::default(),
                    payload.to_vec(),
                    BasicProperties::default(),
                )
                .await?
                .await?;
            assert_eq!(confirm, Confirmation::NotRequested);
        }
    })
}
*/
*/
