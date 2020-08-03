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
