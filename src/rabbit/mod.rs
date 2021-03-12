//! Handles all communication to RabbitMQ
//! This heavily relies on the lapin crate under the hood
//!
//! The key functionality of this module is the `RabbitMessage` trait, which defines an encode/decode interface for all rabbit messages
//! The idea here is we only send defined messages over RabbitMQ, which means we know how to interpret their meaning

pub mod deployment_message;
pub mod log_message;
pub mod sysinfo_message;
pub mod util;
pub mod work_request_message;
use async_trait::async_trait;
use futures_util::stream::StreamExt;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use log::error;
use tokio_amqp::*;

/// The interface between Kraken and RabbitMQ
pub struct RabbitBroker {
    /// Connection to the Rabbit Instance (Should be one per device)
    pub conn: lapin::Connection,
}

/// Label Definitions to be used to specify queues for rabbit messages.
pub enum QueueLabel {
    Sysinfo,
    Deployment,
    Log,
}

impl QueueLabel {
    /// Declares a queue within the rabbit instance for messages within the rabbit instance for messages to be posted to
    /// The typical pattern is to use a `QueueLabel` value as the queue name.
    ///
    /// # Arguments
    ///
    /// * `queue_label` - The string which will differentiate the queue messages will go to. Should be unique across the rabbit instance.
    ///
    /// # Examples
    ///
    /// ```
    /// let label = QueueLabel::Sysinfo.as_str();
    /// ```
    pub fn as_str(&self) -> &'static str {
        // cool pattern from https://users.rust-lang.org/t/noob-enum-string-with-symbols-resolved/7668/2
        match *self {
            QueueLabel::Sysinfo => "sysinfo",
            QueueLabel::Deployment => "deployment",
            QueueLabel::Log => "log",
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
                error!("Error establishing conn: {:?}", e);
                None
            }
        }
    }

    /// Declares a queue within the rabbit instance for messages within the rabbit instance for messages to be posted to
    /// The typical pattern is to use a `QueueLabel` value as the queue name.
    ///
    /// # Arguments
    ///
    /// * `queue_label` - The string which will differentiate the queue messages will go to. Should be unique across the rabbit instance.
    ///
    /// # Examples
    ///
    /// ```
    /// let broker = match RabbitBroker::new(&addr).await {
    /// Some(b) => b,
    /// None => panic!("Could not establish rabbit connection"),
    /// };
    /// broker.declare_queue(QueueLabel::Sysinfo.as_str()).await;
    /// ```
    pub async fn declare_queue(&self, queue_label: &str) -> bool {
        // Channels are cheap, and de-allocate when out of scope
        let chan = &self
            .conn
            .create_channel()
            .await
            .expect("Could not create rabbit channel for queue declaration");
        chan.queue_declare(
            queue_label,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .is_ok()
    }

    /// Gets a new publisher for the existing broker channel. This should not be shared between threads.
    ///
    /// # Examples
    ///
    /// ```
    /// let broker = match RabbitBroker::new(&addr).await {
    /// Some(b) => b,
    /// None => panic!("Could not establish rabbit connection"),
    /// };
    /// broker.get_channel().await;
    /// ```
    pub async fn get_channel(&self) -> lapin::Channel {
        self.conn
            .create_channel()
            .await
            .expect("Could not create rabbit channel via connection")
    }

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

    /// Sends an ack to a message
    async fn ack(channel: &lapin::Channel, tag: u64) {
        channel
            .basic_ack(tag, BasicAckOptions::default())
            .await
            .expect("ack")
    }

    /// Consumes a queue data stream and calls handler on each incoming message
    ///
    /// # Arguments
    ///
    /// * `consumer_tag` - Label for the consumer of the queue
    /// * `queue_label` - Label for the queue to consume, typically a QueueLabel::<T>.to_str()
    /// * `handler` - A function to call on incoming messages
    ///
    /// # Examples
    ///
    /// ```
    /// let broker = match RabbitBroker::new(&addr).await {
    /// Some(b) => b,
    /// None => panic!("Could not establish rabbit connection"),
    /// };
    ///
    /// let handler = |data: Vec<u8>| {
    ///     info!("{}", data);
    /// };
    ///
    /// let consumer = tokio::spawn(async move {
    ///     broker.consume_queue('1234', QueueLabel::Sysinfo.as_str(), handler).await
    /// });
    /// ```
    pub async fn consume_queue<F>(&self, consumer_tag: &str, queue_label: &str, handler: F)
    where
        F: Fn(Vec<u8>),
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

    /// Returns a queue consumer to be used incrementally in conjucntion with rabbit::util::try_fetch_queue_item
    ///
    /// # Arguments
    ///
    /// * `consumer_tag` - Label for the consumer of the queue
    /// * `queue_label` - Label for the queue to consume, typically a QueueLabel::<T>.to_str()
    ///
    /// # Examples
    ///
    /// ```
    /// let broker = match RabbitBroker::new(&addr).await {
    /// Some(b) => b,
    /// None => panic!("Could not establish rabbit connection"),
    /// };
    ///
    /// let consumer = broker.consume_queue('1234', QueueLabel::Sysinfo.as_str()).await
    /// ```
    pub async fn consume_queue_incr(
        &self,
        consumer_tag: &str,
        queue_label: &str,
    ) -> lapin::Consumer {
        let consumer_channel = self.get_channel().await;
        let consumer = Self::get_consumer(&consumer_channel, queue_label, consumer_tag).await;
        consumer
    }
}

#[async_trait]
/// Trait to pack system identifier and data into a vector to be sent by rabbit,
/// and unpack a Vec<u8> to a string for easy processing
pub trait RabbitMessage<T> {
    /// For internal use by the RabbitMessage::send function
    fn build_message(&self) -> Vec<u8>;
    // TODO write unit tests to enforce the send <-> deconstruct relationship
    /// Takes a packet from T::send and turns it back into a T
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, T);
    /// Wrapper around lapin::Channel::basic_publish
    async fn send(&self, channel: &lapin::Channel, tag: &str) -> bool
    where
        T: 'async_trait,
    {
        channel
            .basic_publish(
                "",
                tag,
                BasicPublishOptions::default(),
                self.build_message(),
                BasicProperties::default(),
            )
            .await
            .is_ok()
    }
}
