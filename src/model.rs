use juniper;
use serde::{Deserialize, Serialize};
use std::string::ToString; // for strum enum to string
use strum_macros::{Display, EnumString};

#[derive(Serialize, Debug, Deserialize, Clone, juniper::GraphQLEnum)]
pub enum ServiceStatus {
    OK,
    ERRORED,
}

// Specify GraphQL type with field resolvers (i.e. no computed resolvers)
#[derive(Serialize, Debug, Deserialize, Clone, juniper::GraphQLObject)]
#[graphql(description = "A Service installed on the device to support the platform")]
pub struct Service {
    name: String,
    version: String,
    service_url: String,
    status: ServiceStatus,
}

impl Service {
    pub fn new(name: &str, version: &str, service_url: &str, status: ServiceStatus) -> Service {
        Service {
            name: name.to_owned(),
            version: version.to_owned(),
            service_url: service_url.to_owned(),
            status,
        }
    }
}

//#[derive(juniper::GraphQLObject)]
#[derive(Serialize, Debug, Deserialize)]
pub struct Node {
    pub id: String,
    pub model: String,
    uptime: u64,
    ram_free: u64,
    ram_used: u64,
    load_avg_5: f32,
    pub application_instances: Vec<String>,
    services: Vec<Service>,
}

impl Clone for Node {
    fn clone(&self) -> Self {
        let mut node = Node {
            id: self.id.to_owned(),
            model: self.model.to_owned(),
            ram_free: self.ram_free,
            ram_used: self.ram_used,
            load_avg_5: self.load_avg_5,
            uptime: self.uptime,
            application_instances: Vec::new(),
            services: Vec::new(),
        };
        for service in self.services.iter() {
            node.services.push(service.clone());
        }
        for app in self.application_instances.iter() {
            node.application_instances.push(app.clone());
        }

        node
    }
}

// non-gql impl block for Node
// TODO rename so this makes more sense (is really node-info or something)
impl Node {
    pub fn new(
        id: &str,
        model: &str,
        uptime: u64,
        ram_free: u64,
        ram_used: u64,
        load_avg_5: f32,
    ) -> Node {
        Node {
            id: id.to_owned(),
            model: model.to_owned(),
            uptime,
            ram_free,
            ram_used,
            load_avg_5,
            application_instances: vec![],
            services: vec![],
        }
    }

    pub fn uptime(&self) -> u64 {
        self.uptime
    }

    pub fn ram_free(&self) -> u64 {
        self.ram_free
    }

    pub fn ram_used(&self) -> u64 {
        self.ram_used
    }

    pub fn load_avg_5(&self) -> f32 {
        self.load_avg_5
    }

    pub fn current_ram_percent(&self) -> f64 {
        ((self.ram_used as f64) / ((self.ram_free as f64) + (self.ram_used as f64))) as f64
    }

    pub fn set_id(&mut self, id: &str) -> () {
        self.id = id.to_owned();
    }

    /// update the node values
    pub fn update(&mut self, ram_free: u64, ram_used: u64, uptime: u64, load_avg_5: f32) -> () {
        self.ram_free = ram_free;
        self.ram_used = ram_used;
        self.uptime = uptime;
        self.load_avg_5 = load_avg_5;
    }

    /// Parse a Node from a Vec<String> of a rabbitMQ sysinfo message
    pub fn from_incomplete(
        id: &str,
        model: Option<&str>,
        ram_free: Option<u64>,
        ram_used: Option<u64>,
        uptime: Option<u64>,
        load_avg_5: Option<f32>,
        _apps: Option<Vec<String>>,
        _services: Option<Vec<Service>>,
    ) -> Node {
        // TODO find a safer way to do this
        let n = Node::new(
            id,
            match model {
                Some(m) => m,
                None => "placeholder_model",
            },
            match uptime {
                Some(r) => r,
                None => 0,
            },
            match ram_free {
                Some(r) => r,
                None => 0,
            },
            match ram_used {
                Some(r) => r,
                None => 0,
            },
            match load_avg_5 {
                Some(r) => r,
                None => 0.0,
            },
        );

        /*
        TODO rm (node coming from a message probably doesn't have apps or services)
        if let Some(apps) = apps {
            let app_iter = apps.into_iter();

            while let Some(app) = app_iter.next() {
                n.add_application_instance(&app);
            }
        }

        if let Some(service) = apps {
            let app_iter = apps.into_iter();

            while let Some(app) = app_iter.next() {
                n.add_application_instance(&app);
            }
        }
        */

        n
    }

    pub fn add_service(&mut self, service: Service) -> () {
        self.services.push(service);
    }

    pub fn add_application_instance(&mut self, app: &str) -> () {
        self.application_instances.push(app.to_owned());
    }
}

#[derive(
    Serialize, Deserialize, Debug, Display, Clone, PartialEq, juniper::GraphQLEnum, EnumString,
)]
pub enum ApplicationStatus {
    REQUESTED,
    INITIALIZED,
    RETRIEVING,
    BUILDING,
    TESTING,
    DEPLOYING,
    DEPLOYED,
    ERRORED,
}

#[derive(Serialize, Debug, Deserialize, Clone, juniper::GraphQLObject)]
#[graphql(description = "Metadata pertaining to the Orchestrator User Interface")]
pub struct OrchestratorInterface {
    src_url: Option<String>,
    pub cloned_commit: Option<String>,
    status: ApplicationStatus,
}

impl OrchestratorInterface {
    pub fn new(
        src_url: Option<String>,
        cloned_commit: Option<String>,
        status: ApplicationStatus,
    ) -> OrchestratorInterface {
        OrchestratorInterface {
            src_url: src_url.to_owned(),
            cloned_commit: cloned_commit.to_owned(),
            status,
        }
    }

    pub fn update_status(&mut self, new_status: ApplicationStatus) {
        self.status = new_status;
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, juniper::GraphQLObject)]
#[graphql(description = "Information about the Platform's Orchestration State")]
pub struct Orchestrator {
    pub ui: OrchestratorInterface,
    pub services: Option<Vec<Service>>,
}

impl Orchestrator {
    pub fn new(interface: OrchestratorInterface) -> Orchestrator {
        Orchestrator {
            ui: interface,
            services: None,
        }
    }

    pub fn add_service(&mut self, service: Service) {
        match &mut self.services {
            Some(v) => v.push(service),
            None => self.services = Some(vec![service]),
        }
    }
}

#[derive(Serialize, Debug, Deserialize)]
pub struct Deployment {
    pub id: String,
    pub src_url: String,
    pub version: String,
    pub commit: String,
    pub status: ApplicationStatus,
    pub results_url: String,
    pub deployment_url: String,
    pub instances: Vec<Option<ApplicationInstance>>,
}

impl Deployment {
    pub fn new(
        id: &str,
        src_url: &str,
        version: &str,
        commit: &str,
        status: ApplicationStatus,
        results_url: &str,
        deployment_url: &str,
        instances: &Vec<Option<ApplicationInstance>>,
    ) -> Deployment {
        Deployment {
            id: id.to_owned(),
            src_url: src_url.to_owned(),
            version: version.to_owned(),
            commit: commit.to_owned(),
            status,
            results_url: results_url.to_owned(),
            deployment_url: deployment_url.to_owned(),
            instances: instances.to_owned(),
        }
    }

    pub fn update_status(&mut self, new_status: ApplicationStatus) {
        self.status = new_status;
    }

    // TODO modify to update_instance
    pub fn add_instance(&mut self, instance: ApplicationInstance) {
        self.instances.push(Some(instance));
    }

    pub fn remove_instance(&mut self, _instance_id: &str) {
        // TODO complete
    }
}

impl Clone for Deployment {
    fn clone(&self) -> Self {
        let mut d = Deployment {
            id: self.id.clone(),
            src_url: self.src_url.clone(),
            version: self.version.clone(),
            commit: self.commit.clone(),
            status: self.status.clone(),
            results_url: self.results_url.clone(),
            deployment_url: self.deployment_url.clone(),
            instances: vec![],
        };
        for i in self.instances.iter() {
            d.instances.push(i.clone());
        }

        d
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, juniper::GraphQLObject)]
#[graphql(description = "A Service installed on the device to support the platform")]
pub struct ApplicationInstance {
    instance_id: String,
    node_id: String,
    deployment_id: String,
    status: ServiceStatus,
}

impl ApplicationInstance {
    pub fn new(
        instance_id: &str,
        node_id: &str,
        deployment_id: &str,
        status: ServiceStatus,
    ) -> ApplicationInstance {
        ApplicationInstance {
            instance_id: instance_id.to_owned(),
            node_id: node_id.to_owned(),
            deployment_id: deployment_id.to_owned(),
            status,
        }
    }

    pub fn update_status(&mut self, new_status: ServiceStatus) {
        self.status = new_status;
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Platform {
    pub deployments: Vec<Deployment>,
    pub orchestrator: Orchestrator,
    pub nodes: Vec<Node>,
}

impl Platform {
    pub fn new(
        deployments: &Vec<Deployment>,
        orchestrator: &Orchestrator,
        nodes: &Vec<Node>,
    ) -> Platform {
        Platform {
            deployments: deployments.to_owned(),
            orchestrator: orchestrator.to_owned(),
            nodes: nodes.to_owned(),
        }
    }

    // TODO modify to update_instance
    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn remove_node(&mut self, _node_id: &str) {
        // TODO complete
    }

    // TODO modify to update_instance
    pub fn add_deployment(&mut self, deployment: Deployment) {
        self.deployments.push(deployment);
    }

    pub fn remove_deployment(&mut self, _deployment_id: &str) {
        // TODO complete
    }
}

// TODO Display
/*
impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}, {:?}, {:?}]",
            self.deployments, self.orchestrator, self.nodes
        )
    }
}
*/
