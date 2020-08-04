pub mod deployment_message;
pub mod sysinfo_message;
use async_trait::async_trait;
use futures_util::stream::StreamExt;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use tokio_amqp::*;
/// The interface between Kraken and RabbitMQ
pub struct RabbitBroker {
    /// Connection to the Rabbit Instance (Should be one per device)
    pub conn: lapin::Connection,
}

pub enum QueueLabel {
    Sysinfo,
    Deployment,
}

impl QueueLabel {
    // cool pattern from https://users.rust-lang.org/t/noob-enum-string-with-symbols-resolved/7668/2
    pub fn as_str(&self) -> &'static str {
        match *self {
            QueueLabel::Sysinfo => "sysinfo",
            QueueLabel::Deployment => "deployment",
        }
    }
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
