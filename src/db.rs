use crate::model::{
    ApplicationStatus, Deployment, Node, Orchestrator, OrchestratorInterface, Service,
};
use log::{info, warn};
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

        /*
        let test_services = vec![
            Service::new("RabbitMQ", "0.0.1", ServiceStatus::OK),
            Service::new("Python", "3.6.2", ServiceStatus::ERRORED),
        ];
        */

        //deployments.insert("test".to_owned(), test_services);

        //let test_node = Node::new("test", "EthanShryDesktop", 0, 0, 0, 0.0);

        //nodes.insert("test".to_owned(), test_node);

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

    pub fn insert_node(&mut self, node: &Node) -> Option<Node> {
        self.nodes.insert(node.id.to_owned(), node.to_owned())
    }

    pub fn get_nodes(&self) -> Option<Vec<Node>> {
        info!("Trying to get the nodes from the hashmap");
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

    pub fn add_application_instance_to_node(
        &mut self,
        node_id: &str,
        deployment_id: String,
    ) -> Result<(), ()> {
        info!("Doing app instance insert");
        if let Some(n) = self.nodes.get(node_id) {
            let mut node = n.clone();
            node.application_instances.push(deployment_id);
            self.nodes.insert(node.id.clone(), node.to_owned());
            return Ok(());
        }
        Err(())
    }

    pub fn remove_application_instance_from_nodes(&mut self, deployment_id: &str) {
        info!("Doing app instance remove");
        if let Some(nodes) = self.get_nodes() {
            let key = String::from(deployment_id);
            for node in nodes {
                if node.application_instances.contains(&key) {
                    let mut node = node.clone();
                    node.application_instances.retain(|id| id != deployment_id);
                    self.nodes.insert(node.id.clone(), node.to_owned());
                }
            }
        }
    }
}
