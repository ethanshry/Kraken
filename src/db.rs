use crate::model::{Node, Service, ServiceStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct ManagedDatabase {
    pub db: Arc<Mutex<Database>>,
}

impl ManagedDatabase {
    pub fn new(database: Arc<Mutex<Database>>) -> ManagedDatabase {
        ManagedDatabase { db: database }
    }
}

pub struct Database {
    services: HashMap<String, Vec<Service>>,
    nodes: HashMap<String, Node>,
}

impl Database {
    pub fn new() -> Database {
        let mut services = HashMap::new();
        let mut nodes = HashMap::new();

        let test_services = vec![
            Service::new("RabbitMQ", "0.0.1", ServiceStatus::OK),
            Service::new("Python", "3.6.2", ServiceStatus::ERRORED),
        ];

        services.insert("test".to_owned(), test_services);

        let test_node = Node::new(
            "test",
            "EthanShryDesktop",
            1.0,
            16,
            256,
            2.0,
            32,
            512,
            false,
        );

        nodes.insert("test".to_owned(), test_node);

        Database { services, nodes }
    }

    pub fn get_services(&self, node_id: &str) -> Option<&Vec<Service>> {
        self.services.get(node_id)
    }

    pub fn add_service(&mut self, node_id: &str, service: Service) -> Result<i32, String> {
        match self.services.get_mut(node_id) {
            Some(v) => {
                v.push(service);
                Ok(0)
            }
            None => {
                self.services.insert(node_id.to_owned(), vec![service]);
                Ok(0)
            }
        }
    }

    pub fn get_node(&self, node_id: &str) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    pub fn insert_node(&mut self, node_id: &str, node: Node) -> Option<Node> {
        self.nodes.insert(node_id.to_owned(), node)
    }
}
