//! Storage for platform information for the Orchestrator

use crate::gql_model::{
    ApplicationStatus, Deployment, Node, Orchestrator, OrchestratorInterface, Service,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// An ARC to a Database
pub struct ManagedDatabase {
    pub db: Arc<Mutex<Database>>,
}

impl ManagedDatabase {
    pub fn new(database: Arc<Mutex<Database>>) -> ManagedDatabase {
        ManagedDatabase { db: database }
    }
}

/// The database contains information about the currently running platform.
/// This is managed on the orchestration nodes only
/// The processes which need access to the database will contain an Arc to a Mutex for the Database
#[derive(Serialize, Debug, Deserialize)]
pub struct Database {
    /// Information about the deployments the orchestrator is managing
    deployments: HashMap<String, Deployment>,
    /// Information about the nodes the orchestrator is managing
    nodes: HashMap<String, Node>,
    /// Information about the platform
    orchestrator: Orchestrator,
}

impl Database {
    pub fn new() -> Database {
        let deployments = HashMap::new();
        let nodes = HashMap::new();

        let orchestrator = Orchestrator::new(OrchestratorInterface::new(
            None,
            None,
            ApplicationStatus::Errored,
        ));

        Database {
            deployments,
            nodes,
            orchestrator,
        }
    }

    pub fn get_orchestrator(&self) -> Orchestrator {
        self.orchestrator.clone()
    }

    pub fn update_orchestrator_interface(&mut self, o: &OrchestratorInterface) {
        self.orchestrator.ui = o.to_owned()
    }

    pub fn get_platform_services(&self) -> Option<Vec<Service>> {
        match &self.orchestrator.services {
            Some(services) => match services.len() {
                0 => None,
                _ => self.orchestrator.services.clone(),
            },
            None => None,
        }
    }

    pub fn add_platform_service(&mut self, service: Service) -> Result<i32, String> {
        self.orchestrator.add_service(service);
        Ok(0)
    }

    pub fn get_node(&self, node_id: &str) -> Option<Node> {
        match self.nodes.get(node_id) {
            Some(n) => Some(n.clone()),
            None => None,
        }
    }

    /// Assigns a priority to a node
    /// If a node already has a priority, it will return it
    /// Otherwise, it will assign a node the next lowest priority
    pub fn get_orchestrator_rank(&mut self, node_id: &str) -> u8 {
        if let Some(n) = self.nodes.get(node_id) {
            let mut n = n.clone();
            // node does not have an assigned priority, so figure one out
            let nodes: Vec<_> = self.nodes.iter().collect();
            let mut priorities: [bool; 255] = [false; 255];
            for (_, n) in nodes {
                if let Some(p) = n.orchestration_priority {
                    priorities[p as usize] = true;
                }
            }
            for (i, v) in priorities.iter().enumerate() {
                if *v == false && i != 0 {
                    if let Some(p) = n.orchestration_priority {
                        if p < (i as u8) {
                            return p;
                        }
                    }
                    n.orchestration_priority = Some(i as u8);
                    self.nodes.insert(node_id.to_string(), n);
                    return i as u8;
                }
            }
            // No priority is available
            return 255;
        } else {
            // node is not in DB, so give no priority
            return 255;
        }
    }

    pub fn insert_node(&mut self, node: &Node) -> Option<Node> {
        self.nodes.insert(node.id.to_owned(), node.to_owned())
    }

    pub fn get_nodes(&self) -> Option<Vec<Node>> {
        let mut res = vec![];

        for (_, v) in self.nodes.iter() {
            res.push(v.clone());
        }

        match res.len() {
            0 => None,
            _ => Some(res),
        }
    }

    pub fn get_deployment(&self, deployment_id: &str) -> Option<Deployment> {
        match self.deployments.get(deployment_id) {
            Some(d) => Some(d.clone()),
            None => None,
        }
    }

    pub fn insert_deployment(&mut self, deployment: &Deployment) -> Option<Deployment> {
        self.deployments
            .insert(deployment.id.to_owned(), deployment.to_owned())
    }

    pub fn update_deployment(
        &mut self,
        deployment_id: &str,
        deployment: &Deployment,
    ) -> Option<Deployment> {
        info!("updating {}", deployment.id);
        if !self.deployments.contains_key(deployment_id) {
            warn!(
                "Deployment id: {} does not exist, inserting new deployment instead",
                deployment.id
            );
        }
        self.deployments
            .insert(deployment_id.to_owned(), deployment.to_owned())
    }

    pub fn get_deployments(&self) -> Option<Vec<Deployment>> {
        let mut res = vec![];

        for (_, v) in self.deployments.iter() {
            res.push(v.clone());
        }

        match res.len() {
            0 => None,
            _ => Some(res),
        }
    }

    pub fn add_deployment_to_node(
        &mut self,
        node_id: &str,
        deployment_id: String,
    ) -> Result<(), ()> {
        if let Some(n) = self.nodes.get(node_id) {
            let mut node = n.clone();
            node.deployments.push(deployment_id);
            self.nodes.insert(node.id.clone(), node.to_owned());
            return Ok(());
        }
        Err(())
    }

    pub fn remove_deployment_from_nodes(&mut self, deployment_id: &str) {
        if let Some(nodes) = self.get_nodes() {
            let key = String::from(deployment_id);
            for node in nodes {
                if node.deployments.contains(&key) {
                    let mut node = node.clone();
                    node.deployments.retain(|id| id != deployment_id);
                    self.nodes.insert(node.id.clone(), node.to_owned());
                }
            }
        }
    }

    /// Deletes the specified node and marks all its owned deployments for re-deploy
    pub fn delete_node(&mut self, node_id: &str) -> Option<Node> {
        if let Some(_) = self.get_node(node_id) {
            if let Some(ds) = self.get_deployments() {
                for mut d in ds {
                    if d.node == node_id {
                        d.node = String::from("");
                        d.update_status(&ApplicationStatus::DeploymentRequested);
                        self.update_deployment(&d.id, &d);
                    }
                }
            }
            return self.nodes.remove(node_id);
        }
        None
    }

    pub fn clear(&mut self) {
        self.orchestrator = Orchestrator::new(OrchestratorInterface::new(
            None,
            None,
            ApplicationStatus::Errored,
        ));

        self.nodes.clear();
        self.deployments.clear();
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn db_handles_nodes() {
        let mut db = Database::new();

        let node = Node::new(
            "abcd",
            "temp",
            "192.168.0.55",
            crate::platform_executor::NodeMode::ORCHESTRATOR,
            None,
        );

        db.insert_node(&node);

        assert_eq!(db.nodes.iter().count(), 1);

        assert_eq!(db.get_node("1234"), None);
        assert_eq!(db.get_node("abcd"), Some(node));
    }

    #[test]
    fn db_handles_deployments() {
        let mut db = Database::new();

        let deployment = Deployment::new(
            "1234",
            "http://url.com",
            "main",
            "1.0.0",
            "abcd1234",
            ApplicationStatus::Running,
            "localhost:8000/log/1234",
            "localhost:9000",
            "3000",
            "abcd",
        );

        db.insert_deployment(&deployment);

        assert_eq!(db.get_deployment("abcd"), None);
        assert_eq!(db.get_deployment("1234"), Some(deployment));
    }
}
