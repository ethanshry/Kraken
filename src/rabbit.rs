use amiquip::{Connection, ConsumerOptions, Exchange, QueueDeclareOptions};
use std::iter::FromIterator;

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
pub trait RabbitMessage {
    fn build_message(identifier: &String, packet_data: String) -> Vec<u8>;
    fn deconstruct_message(packet_data: &Vec<u8>) -> Vec<String>;
}

pub struct SysinfoMessage {}

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
