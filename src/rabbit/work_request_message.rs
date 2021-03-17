//! A message requesting a task of an worker role
use crate::rabbit::RabbitMessage;
use std::iter::FromIterator;

/// Defines the type of request we are recieving. This informs the schema the data is packed/sent in
#[derive(Clone, Debug, PartialEq)]
pub enum WorkRequestType {
    RequestDeployment,
    CancelDeployment,
}

impl WorkRequestType {
    pub fn as_str(&self) -> &'static str {
        // cool pattern from https://users.rust-lang.org/t/noob-enum-string-with-symbols-resolved/7668/2
        match *self {
            WorkRequestType::RequestDeployment => "request_deployment",
            WorkRequestType::CancelDeployment => "cancel_deployment",
        }
    }

    pub fn from_str(s: &str) -> WorkRequestType {
        // cool pattern from https://users.rust-lang.org/t/noob-enum-string-with-symbols-resolved/7668/2
        match s {
            "request_deployment" => WorkRequestType::RequestDeployment,
            _ => WorkRequestType::CancelDeployment,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Message Type used when the Orchestrator is requesting work from a node
/// This message has multiple formats, which are represented by the request_type
/// This request_type is then used to encode and decode the rest of the rabbitmq message
pub struct WorkRequestMessage {
    pub request_type: WorkRequestType,
    pub deployment_id: Option<String>, // Only set with WorkRequestType::{RequestDeployment, CancelDeployment}
    pub deployment_url: Option<String>, // Only set with WorkRequestType::RequestDeployment
    pub git_branch: Option<String>,
    pub priority: Option<i16>, // Only set with WorkRequestType::SetPromotionPriority
}

impl WorkRequestMessage {
    pub fn new(
        request_type: WorkRequestType,
        deployment_id: Option<&str>,
        deployment_url: Option<&str>,
        git_branch: Option<&str>,
        priority: Option<i16>,
    ) -> WorkRequestMessage {
        WorkRequestMessage {
            request_type,
            deployment_id: match deployment_id {
                Some(d) => Some(d.to_owned()),
                None => None,
            },
            deployment_url: match deployment_url {
                Some(d) => Some(d.to_owned()),
                None => None,
            },
            git_branch: match git_branch {
                Some(b) => Some(b.to_owned()),
                None => None,
            },
            priority,
        }
    }
}

// TODO figure out how to better represent message types for the Work Queues
// Maybe I should be serializing and deserializing the structs directly?
impl RabbitMessage<WorkRequestMessage> for WorkRequestMessage {
    fn build_message(&self) -> Vec<u8> {
        match self.request_type {
            WorkRequestType::RequestDeployment => format!(
                "{}|{}|{}|{}",
                self.request_type.as_str(),
                self.deployment_id.as_ref().unwrap(),
                self.deployment_url.as_ref().unwrap(),
                self.git_branch.as_ref().unwrap()
            )
            .as_bytes()
            .to_vec(),
            WorkRequestType::CancelDeployment => format!(
                "{}|{}",
                self.request_type.as_str(),
                self.deployment_id.as_ref().unwrap(),
            )
            .as_bytes()
            .to_vec(),
        }
    }

    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, WorkRequestMessage) {
        let res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split('|')
                .map(|s| s.to_string()),
        );

        let request_type = WorkRequestType::from_str(&res[0]);

        let msg = match request_type {
            WorkRequestType::RequestDeployment => WorkRequestMessage::new(
                request_type,
                Some(&res[1]),
                Some(&res[2]),
                Some(&res[3]),
                None,
            ),

            WorkRequestType::CancelDeployment => {
                WorkRequestMessage::new(request_type, Some(&res[1]), None, None, None)
            }
        };

        (res[0].clone(), msg)
    }
}

#[test]
fn workrequestmessage_is_invertible() {
    let left = WorkRequestMessage::new(
        WorkRequestType::RequestDeployment,
        Some("deployid"),
        Some("http://github.com"),
        Some("main"),
        None,
    );
    let data = left.build_message();

    let (_, right) = WorkRequestMessage::deconstruct_message(&data);
    assert_eq!(left, right);
}
