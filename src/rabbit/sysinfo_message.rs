//! A message containing status information for a node on the platform
use crate::rabbit::RabbitMessage;

use std::iter::FromIterator;

#[derive(PartialEq, Debug)]
pub struct SysinfoMessage {
    pub system_identifier: String,
    pub model_name: String,
    pub lan_addr: String,
    pub ram_free: u64,
    pub ram_used: u64,
    pub uptime: u64,
    pub load_avg_5: f32,
}

impl SysinfoMessage {
    pub fn new(system_identifier: &str, model_name: &str, lan_addr: &str) -> SysinfoMessage {
        SysinfoMessage {
            system_identifier: system_identifier.to_owned(),
            model_name: model_name.to_owned(),
            lan_addr: lan_addr.to_owned(),
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
    fn build_message(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.system_identifier,
            self.model_name,
            self.lan_addr,
            self.ram_free,
            self.ram_used,
            self.uptime,
            self.load_avg_5
        )
        .as_bytes()
        .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, SysinfoMessage) {
        let res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split('|')
                .map(|s| s.to_string()),
        );

        let mut msg = SysinfoMessage::new(
            res.get(0).unwrap(),
            res.get(1).unwrap(),
            res.get(2).unwrap(),
        );

        if res.len() == 7 {
            msg.update_message(
                res.get(3)
                    .unwrap_or(&String::from("0"))
                    .parse::<u64>()
                    .unwrap_or(0),
                res.get(4)
                    .unwrap_or(&String::from("0"))
                    .parse::<u64>()
                    .unwrap_or(0),
                res.get(5)
                    .unwrap_or(&String::from("0"))
                    .parse::<u64>()
                    .unwrap_or(0),
                res.get(6)
                    .unwrap_or(&String::from("0"))
                    .parse::<f32>()
                    .unwrap_or(0.0),
            );
        }

        (res[0].clone(), msg)
    }
}

#[test]
fn sysinfomessage_is_invertible() {
    let mut left = SysinfoMessage::new("id", "test_model", "127.0.0.1");
    left.update_message(15, 15, 15, 1.5);
    let data = left.build_message();

    let (id, right) = SysinfoMessage::deconstruct_message(&data);
    assert_eq!(id, "id");
    assert_eq!(left, right);
}
