//! A message describing the status of an active deployment

use crate::gql_model::ApplicationStatus;
use crate::rabbit::RabbitMessage;
use std::iter::FromIterator;
use std::str::FromStr;

#[derive(PartialEq, Debug)]
pub struct DeploymentMessage {
    pub system_identifier: String,
    pub deployment_id: String,
    pub deployment_status: ApplicationStatus,
    pub deployment_status_description: String,
}

impl DeploymentMessage {
    pub fn new(system_identifier: &str, deployment_id: &str) -> DeploymentMessage {
        DeploymentMessage {
            system_identifier: system_identifier.to_owned(),
            deployment_id: deployment_id.to_owned(),
            deployment_status: ApplicationStatus::DeploymentRequested,
            deployment_status_description: "".to_owned(),
        }
    }

    pub fn update_message(&mut self, s: ApplicationStatus, descr: &str) {
        self.deployment_status = s;
        self.deployment_status_description = descr.to_owned();
    }
}

impl RabbitMessage<DeploymentMessage> for DeploymentMessage {
    fn build_message(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}",
            self.system_identifier,
            self.deployment_id,
            self.deployment_status,
            self.deployment_status_description
        )
        .as_bytes()
        .to_vec()
    }
    fn deconstruct_message(packet_data: &Vec<u8>) -> (String, DeploymentMessage) {
        let res = Vec::from_iter(
            String::from_utf8_lossy(packet_data)
                .split('|')
                .map(|s| s.to_string()),
        );

        let mut msg = DeploymentMessage::new(&res[0], &res[1]);

        if res.len() == 4 {
            msg.update_message(ApplicationStatus::from_str(&res[2]).unwrap(), &res[3]);
        }

        (res[0].clone(), msg)
    }
}

#[test]
fn deploymentmessage_is_invertible() {
    let mut left = DeploymentMessage::new("sysid", "deployid");
    left.update_message(ApplicationStatus::Running, "deployment is running");
    let data = left.build_message();

    let (id, right) = DeploymentMessage::deconstruct_message(&data);
    assert_eq!(id, "sysid");
    assert_eq!(left, right);
}
