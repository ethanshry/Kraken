use crate::model::{
    ApplicationStatus, Deployment, Node, Orchestrator, OrchestratorInterface, Service,
};
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
pub struct Database {
    deployments: HashMap<String, Deployment>,
    nodes: HashMap<String, Node>,
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
            ApplicationStatus::ERRORED,
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

    pub fn update_orchestrator_interface(&mut self, o: OrchestratorInterface) {
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

    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    pub fn insert_node(&mut self, node: Node) -> Option<Node> {
        println!("inserting {}", node.id);
        self.nodes.insert(node.id.to_owned(), node)
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

    pub fn get_deployment(&self, deployment_id: &str) -> Option<&Deployment> {
        self.deployments.get(deployment_id)
    }

    pub fn insert_deployment(&mut self, deployment: Deployment) -> Option<Deployment> {
        println!("inserting {}", deployment.id);
        self.deployments
            .insert(deployment.id.to_owned(), deployment)
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
}
