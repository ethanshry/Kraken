//! A message containing log data from an active deployment
use crate::rabbit::RabbitMessage;

use std::iter::FromIterator;

#[derive(PartialEq, Debug)]
pub struct LogMessage {
    pub deployment_identifier: String,
    pub message: String,
}

impl LogMessage {
    pub fn new(deployment_identifier: &str) -> LogMessage {
        LogMessage {
            deployment_identifier: deployment_identifier.to_owned(),
            message: String::from(""),
        }
    }

    pub fn update_message(&mut self, message: &str) {
        self.message = message.to_string();
    }
}

impl RabbitMessage<LogMessage> for LogMessage {
    fn build_message(&self) -> Vec<u8> {
        format!("{}|{}", self.deployment_identifier, self.message)
            .as_bytes()
            .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, LogMessage) {
        let mut res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split('|')
                .map(|s| s.to_string()),
        );

        let mut msg = LogMessage::new(res.get(0).unwrap());

        let mut message = String::from("");

        res.remove(0);

        for piece in res.iter() {
            message.push_str(piece);
        }

        msg.update_message(&message);

        (msg.deployment_identifier.clone(), msg)
    }
}

#[test]
fn logmessage_is_invertible() {
    let mut left = LogMessage::new("deployid");
    left.update_message("deployment is running");
    let data = left.build_message();

    let (id, right) = LogMessage::deconstruct_message(&data);
    assert_eq!(id, "deployid");
    assert_eq!(left, right);
}
