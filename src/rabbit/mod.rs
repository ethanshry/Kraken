pub mod deployment_message;
pub mod sysinfo_message;
use async_trait::async_trait;
use futures_util::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use std::future::Future;
use std::iter::FromIterator;
use std::str::FromStr;
use std::string::ToString;
use tokio_amqp::*;
/// The interface between Kraken and RabbitMQ
pub struct RabbitBroker {
    /// Connection to the Rabbit Instance (Should be one per device)
    pub conn: lapin::Connection,
}

impl RabbitBroker {
    /// Creates a new RabbitBroker, returns None if no rabbit connection could be made
    /// This indicates that the machine needs to spin up a rabbit instance
    pub async fn new(addr: &str) -> Option<RabbitBroker> {
        let conn = Connection::connect(&addr, ConnectionProperties::default().with_tokio()).await;
        match conn {
            Ok(c) => Some(RabbitBroker { conn: c }),
            Err(e) => {
                println!("Error establishing conn: {:?}", e);
                None
            }
        }
    }

    /// Declares a queue to which rabbitmq messages can be published
    pub async fn declare_queue(&self, queue_label: &str) -> bool {
        // Channels are cheap, and de-allocate when out of scope
        let chan = &self
            .conn
            .create_channel()
            .await
            .expect("Could not create rabbit channel for queue declaration");
        match chan
            .queue_declare(
                queue_label,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
        {
            Ok(_) => true,
            Err(_) => false, // Don't worry about faliures right now
        }
    }

    /// Gets a new publisher for the existing broker channel. This should happen on the thread it is operating on
    pub async fn get_channel(&self) -> lapin::Channel {
        self.conn
            .create_channel()
            .await
            .expect("Could not create rabbit channel via connection")
    }

    /*
    /// Gets a consumer of a queue
    pub async fn get_consumer(
        &self,
        queue_label: &str,
        consumer_tag: &str,
    ) -> (lapin::Channel, lapin::Consumer) {
        let chan: lapin::Channel = self
            .conn
            .create_channel()
            .await
            .expect("cannot build channel");
        chan.confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("Cannot confirm options");

        (
            chan,
            chan.basic_consume(
                queue_label,
                consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("cannot create consumer"),
        )
    }
    */

    /// Converts a queue into a consumer
    pub async fn get_consumer(
        channel: &lapin::Channel,
        queue_label: &str,
        consumer_tag: &str,
    ) -> lapin::Consumer {
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("Cannot confirm options");
        channel
            .basic_consume(
                queue_label,
                consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("cannot create consumer")
    }

    pub async fn ack(channel: &lapin::Channel, tag: u64) -> () {
        channel
            .basic_ack(tag, BasicAckOptions::default())
            .await
            .expect("ack")
    }

    // TODO dive into dyn
    pub async fn consume_queue<F>(&self, consumer_tag: &str, queue_label: &str, handler: F)
    where
        F: Fn(Vec<u8>) -> (),
    {
        let consumer_channel = self.get_channel().await;
        let mut consumer = Self::get_consumer(&consumer_channel, queue_label, consumer_tag).await;
        while let Some(delivery) = consumer.next().await {
            let (channel, delivery) = delivery.expect("error in consumer");
            let data = delivery.data;
            RabbitBroker::ack(&channel, delivery.delivery_tag).await;
            handler(data);
        }
    }
}

#[async_trait]
/// Trait to pack system identifier and data into a vector to be sent by rabbit,
/// and unpack a Vec<u8> to a string for easy processing
pub trait RabbitMessage<T> {
    fn build_message(&self) -> Vec<u8>;
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, T);
    async fn send(&self, channel: &lapin::Channel, tag: &str) -> bool
    where
        T: 'async_trait,
    {
        match channel
            .basic_publish(
                "",
                tag,
                BasicPublishOptions::default(),
                self.build_message(),
                BasicProperties::default(),
            )
            .await
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

// TODO build in default send functions here
/*
use amiquip::{Connection, ConsumerOptions, Exchange, QueueDeclareOptions};
use std::iter::FromIterator;
use std::str::FromStr;
use std::string::ToString;

use crate::model::ApplicationStatus;

/// The interface between Kraken and RabbitMQ
pub struct RabbitBroker {
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

    /// Gets a consumer of a queue
    pub fn get_consumer(&self, queue_label: &str) -> amiquip::Consumer {
        self.channel
            .queue_declare(queue_label, QueueDeclareOptions::default())
            .expect("Could not establish consumer queue")
            .consume(ConsumerOptions::default())
            .expect("Could not consume the queue")
    }
}

/// Trait to pack system identifier and data into a vector to be sent by rabbit,
/// and unpack a Vec<u8> to a string for easy processing
pub trait RabbitMessage<T> {
    fn build_message(&self, identifier: &String) -> Vec<u8>;
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, T);
}

// TODO implement format
pub struct SysinfoMessage {
    pub ram_free: u64,
    pub ram_used: u64,
    pub uptime: u64,
    pub load_avg_5: f32,
}

impl SysinfoMessage {
    pub fn new() -> SysinfoMessage {
        SysinfoMessage {
            ram_free: 0,
            ram_used: 0,
            uptime: 0,
            load_avg_5: 0.0,
        }
    }

    pub fn update_message(&mut self, ram_free: u64, ram_used: u64, uptime: u64, load_avg_5: f32) {
        self.ram_free = ram_free;
        self.ram_used = ram_used;
        self.uptime = uptime;
        self.load_avg_5 = load_avg_5;
    }
}

impl RabbitMessage<SysinfoMessage> for SysinfoMessage {
    fn build_message(&self, identifier: &String) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{}",
            identifier, self.ram_free, self.ram_used, self.uptime, self.load_avg_5
        )
        .as_bytes()
        .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, SysinfoMessage) {
        let res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split("|")
                .map(|s| s.to_string()),
        );

        let mut msg = SysinfoMessage::new();

        if res.len() == 5 {
            msg.update_message(
                res.get(1).unwrap().parse::<u64>().unwrap(),
                res.get(2).unwrap().parse::<u64>().unwrap(),
                res.get(3).unwrap().parse::<u64>().unwrap(),
                res.get(4).unwrap().parse::<f32>().unwrap(),
            );
        }

        (res[0].clone(), msg)
    }
}

pub struct DeploymentMessage {
    pub deployment_id: String,
    pub deployment_status: ApplicationStatus,
    pub deployment_status_description: String,
}

impl DeploymentMessage {
    pub fn new(deployment_id: &str) -> DeploymentMessage {
        DeploymentMessage {
            deployment_id: deployment_id.to_owned(),
            deployment_status: ApplicationStatus::INITIALIZED,
            deployment_status_description: "".to_owned(),
        }
    }

    pub fn update_message(&mut self, s: ApplicationStatus, descr: &str) {
        self.deployment_status = s.to_owned();
        self.deployment_status_description = descr.to_owned();
    }
}

impl RabbitMessage<DeploymentMessage> for DeploymentMessage {
    fn build_message(&self, identifier: &String) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}",
            identifier,
            self.deployment_id,
            self.deployment_status,
            self.deployment_status_description
        ).as_bytes()
        .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, DeploymentMessage) {
        let res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split("|")
                .map(|s| s.to_string()),
        );

        let mut msg = DeploymentMessage::new(&res[1]);

        if res.len() == 4 {
            msg.update_message(ApplicationStatus::from_str(&res[2]).unwrap(), &res[3]);
        }

        (res[0].clone(), msg)
    }
}
*/
