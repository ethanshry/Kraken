//! Defines the model and resolvers for much of the GraphQL Schema

use crate::platform_executor::NodeMode;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::{
    string::ToString,
    time::{Duration, UNIX_EPOCH},
}; // for strum enum to string
use strum_macros::{Display, EnumString};

/// Statuses for Platform Services
#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, juniper::GraphQLEnum)]
pub enum ServiceStatus {
    OK,
    ERRORED,
}

/// A Service Provided by the Kraken Platform
#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, juniper::GraphQLObject)]
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

/// A Node on the Platform
/// Nodes contain multiple `executor`s, which handle platform tasks
#[derive(Serialize, Debug, Deserialize, PartialEq)]
pub struct Node {
    /// Unique ID for the node
    pub id: String,
    /// Descriptive string for the node (i.e. x86-64 Ubuntu 20.04, desktop-computer, Raspberry Pi 4, etc)
    pub model: String,
    /// The IP address of the node
    pub addr: String,
    /// The mode the node is operating in
    pub mode: NodeMode,
    /// The prirority this node holds for orchestration
    /// * `Some(1)` - This node is the platform orchestrator
    /// * `Some(n)` - This node is a rollover candidate, lowest number = highest priority
    /// * `None` - This node has not been assigned a rollover priority
    pub orchestration_priority: Option<u8>,
    /// The time the node has been running
    uptime: u64,
    /// How much ram is free on the device, in...bytes?
    ram_free: u64,
    /// How much ram is in use on the device, in...bytes?
    ram_used: u64,
    /// Average load on the CPU over last five minutes
    load_avg_5: f32,
    /// List of deployment IDs associated with this Node
    pub deployments: Vec<String>,
    /// List of platform services associated with this Node
    services: Vec<Service>,
    /// The last time this Node's data was updated
    pub update_time: u64,
}

impl Clone for Node {
    fn clone(&self) -> Self {
        let mut node = Node {
            id: self.id.to_owned(),
            model: self.model.to_owned(),
            addr: self.addr.clone(),
            mode: self.mode.clone(),
            orchestration_priority: self.orchestration_priority,
            ram_free: self.ram_free,
            ram_used: self.ram_used,
            load_avg_5: self.load_avg_5,
            uptime: self.uptime,
            deployments: Vec::new(),
            services: Vec::new(),
            update_time: self.update_time,
        };
        for service in self.services.iter() {
            node.services.push(service.clone());
        }
        for app in self.deployments.iter() {
            node.deployments.push(app.clone());
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
        addr: &str,
        mode: NodeMode,
        orchestration_priority: Option<u8>,
    ) -> Node {
        Node {
            id: id.to_owned(),
            model: model.to_owned(),
            addr: addr.to_owned(),
            mode: mode,
            orchestration_priority: orchestration_priority,
            uptime: 0,
            ram_free: 0,
            ram_used: 0,
            load_avg_5: 0.0,
            deployments: vec![],
            services: vec![],
            update_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::new(0, 0))
                .as_secs(),
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

    pub fn set_id(&mut self, id: &str) {
        self.id = id.to_owned();
    }

    /// update the node values
    pub fn update(&mut self, ram_free: u64, ram_used: u64, uptime: u64, load_avg_5: f32) {
        self.update_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0))
            .as_secs();
        self.ram_free = ram_free;
        self.ram_used = ram_used;
        self.uptime = uptime;
        self.load_avg_5 = load_avg_5;
    }

    /// Parse a Node from a Vec<String> of a rabbitMQ sysinfo message
    pub fn from_incomplete(
        id: &str,
        model: Option<&str>,
        addr: &str,
        ram_free: Option<u64>,
        ram_used: Option<u64>,
        uptime: Option<u64>,
        load_avg_5: Option<f32>,
        _apps: Option<Vec<String>>,
        _services: Option<Vec<Service>>,
    ) -> Node {
        // TODO find a cleaner way to do this
        // Would like to model.unwrap_or("placeholder") or similiar
        let mut n = Node::new(
            id,
            match model {
                Some(m) => m,
                None => "placeholder_model",
            },
            addr,
            NodeMode::ORCHESTRATOR, // TODO figure out how mode is useful
            None,
        );
        if let Some(u) = uptime {
            n.uptime = u;
        }
        if let Some(r) = ram_free {
            n.ram_free = r;
        }
        if let Some(r) = ram_used {
            n.ram_used = r;
        }
        if let Some(l) = load_avg_5 {
            n.load_avg_5 = l;
        }
        n
    }

    pub fn add_service(&mut self, service: Service) {
        self.services.push(service);
    }

    pub fn add_application_instance(&mut self, app: &str) {
        self.deployments.push(app.to_owned());
    }
}

/// States an application (deployment) can be in
#[derive(
    Serialize, Deserialize, Debug, Display, Clone, PartialEq, juniper::GraphQLEnum, EnumString,
)]
pub enum ApplicationStatus {
    /// User has requested a deployment
    /// This state is used by the orchestrator to initiate new deployments
    DeploymentRequested,
    /// Ensuring deployment has a valid configuration
    ValidatingDeploymentData,
    /// Deployment data is being cloned to the target Node
    RetrievingApplicationData,
    /// The deployment is being assigned to a Worker on the Platform
    DelegatingDeployment,
    /// The Worker is building the docker image
    BuildingDeployment,
    /// Unused, intended for CI operations
    TestingDeployment,
    /// Docker image has been built, rolling out container
    Deploying,
    /// Deployment is running normally
    Running,
    /// Deployment has errored somewhere along the build process, or container has errored out
    Errored,
    /// The deployment has been requested to be killed. Used by the Orchestrator to Delegate Destruction
    DestructionRequested,
    /// The orchestrator is dispatching a WorkRequestMessage to kill the Deployment
    DelegatingDestruction,
    /// The Worker responsible for the Deployment is killing the Docker Container
    DestructionInProgress,
    /// The Deployment has succesfully been destroyed
    Destroyed,
    /// The User has requested the deployment be updated from the remote source
    /// This will trigger a Destruction, followed by a Deplotment Request
    UpdateRequested,
}

/// An Application Status coupled with a time for the update
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TemporalApplicationStatus {
    pub status: ApplicationStatus,
    /// Unix timestamp for the ApplicationStatus update. This is typically assigned
    /// upon insertion into the orchestrator database, so the time probably lags behind when it actually
    /// would have entered that state
    pub time: u64,
}

/// Information associated with a Kraken-UI instance
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

/// Orchestrator-Specific Metadata
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

/// Information related to a specific Deployment
#[derive(Serialize, Debug, Deserialize, PartialEq)]
pub struct Deployment {
    /// Unique ID for this Deployment
    pub id: String,
    /// The github.com url this deployment comes from (NOTE: NOT the clone link)
    pub src_url: String,
    pub git_branch: String,
    pub version: String,
    /// The git commit associated with the current version of the Deployment
    pub commit: String,
    /// The current status of this Deplyment
    pub status: (ApplicationStatus, SystemTime),
    /// A history of statuses for this Deployment
    pub status_history: Vec<(ApplicationStatus, SystemTime)>,
    pub results_url: String,
    /// The URL the Deployment can be accessed on
    pub deployment_url: String,
    pub port: String,
    /// The ID of the Node responsible for the Deployment
    pub node: String,
    /// Bytes the container is using in Disk, this is rarely appropriately reported by docker
    pub size: i32,
    /// Current memory usage for the container, in Mb
    pub mem_mb: i32,
    /// Maximum memory usage for the container, in Mb
    pub max_mem_mb: i32,
    /// CPU utilization, as a percentage
    pub cpu_usage: f32,
}

impl Deployment {
    pub fn new(
        id: &str,
        src_url: &str,
        git_branch: &str,
        version: &str,
        commit: &str,
        status: ApplicationStatus,
        results_url: &str,
        deployment_url: &str,
        port: &str,
        node: &str,
    ) -> Deployment {
        Deployment {
            id: id.to_owned(),
            src_url: src_url.to_owned(),
            git_branch: git_branch.to_owned(),
            version: version.to_owned(),
            commit: commit.to_owned(),
            status: (status, SystemTime::now()),
            status_history: vec![],
            results_url: results_url.to_owned(),
            deployment_url: deployment_url.to_owned(),
            port: port.to_owned(),
            node: node.to_owned(),
            size: 0,
            mem_mb: 0,
            max_mem_mb: 0,
            cpu_usage: 0.0,
        }
    }

    /// Updates the node's status, keeping a record of status history
    pub fn update_status(&mut self, new_status: &ApplicationStatus) {
        // Only update the status if it is actually new
        if *new_status != self.status.0 {
            match self.status.0 {
                ApplicationStatus::DestructionRequested
                | ApplicationStatus::UpdateRequested
                | ApplicationStatus::DeploymentRequested => {
                    return;
                }
                _ => {
                    self.status_history.push(self.status.clone());
                    self.status = (new_status.clone(), SystemTime::now());
                }
            }
        }
    }

    /// Updates relevant docker container information
    pub fn update_container_status(&mut self, stats: &crate::docker::ContainerStatus) {
        self.size = stats.size;
        self.mem_mb = stats.mem_mb;
        self.max_mem_mb = stats.mem_max_mb;
        self.cpu_usage = stats.cpu_usage;
    }
}

impl Clone for Deployment {
    fn clone(&self) -> Self {
        Deployment {
            id: self.id.clone(),
            src_url: self.src_url.clone(),
            git_branch: self.git_branch.clone(),
            version: self.version.clone(),
            commit: self.commit.clone(),
            status: self.status.clone(),
            status_history: self.status_history.clone(),
            results_url: self.results_url.clone(),
            deployment_url: self.deployment_url.clone(),
            port: self.port.clone(),
            node: self.node.clone(),
            size: self.size,
            mem_mb: self.mem_mb,
            max_mem_mb: self.max_mem_mb,
            cpu_usage: self.cpu_usage,
        }
    }
}

/// Data about the platform
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

    pub fn add_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn add_deployment(&mut self, deployment: Deployment) {
        self.deployments.push(deployment);
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn deployment_appropriately_updates_status() {
        let mut d = Deployment::new(
            "12345",
            "http://github.com",
            "main",
            "1.0.0",
            "abc123",
            ApplicationStatus::Running,
            "http://123.456",
            "http://123.456",
            "5000",
            "54321",
        );

        assert!(d.status_history.len() == 0);

        d.update_status(&ApplicationStatus::Running);

        // Status is same as initial, so history should not change
        assert!(d.status_history.len() == 0);

        d.update_status(&ApplicationStatus::Errored);

        // Status is same as initial, so history should change
        assert!(d.status_history.len() == 1);
        assert!(d.status_history[0].0 == ApplicationStatus::Running);
    }
}
