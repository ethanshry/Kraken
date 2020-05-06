use crate::db::Database;
use crate::model::{
    ApplicationInstance, ApplicationStatus, Deployment, Node, Orchestrator, OrchestratorInterface,
    Platform, Service,
};
use juniper::{FieldError, FieldResult};

impl juniper::Context for Database {}

// Specify computed resolvers for GQL type
// Note only visible resolvers can be here, other properties must be in a different impl block
#[juniper::object(context = Database)]
#[graphql(description = "A physical devide which is a member of the platform")]
impl Node {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn model(&self) -> &str {
        self.model.as_str()
    }

    pub fn uptime(&self) -> i32 {
        self.uptime() as i32
    }

    pub fn ram_free(&self) -> i32 {
        self.ram_free() as i32
    }

    pub fn ram_used(&self) -> i32 {
        self.ram_used() as i32
    }

    /// Five minute load average (<1 indicated processes are not waiting for resources)
    pub fn load_avg_5(&self) -> f64 {
        self.load_avg_5() as f64
    }

    fn current_ram_percent(&self) -> f64 {
        self.current_ram_percent()
    }

    fn application_instances(&self) -> Vec<String> {
        self.application_instances.clone()
    }

    // TODO figure out what is going on with this
    /*
    fn services(&self, context: &Database) -> Option<&Vec<Service>> {
        context.get_platform_services()
    }
    */
}

#[juniper::object(context = Database)]
#[graphql(description = "A physical devide which is a member of the platform")]
impl Deployment {
    fn id(&self) -> &str {
        self.id.as_str()
    }

    fn src_url(&self) -> &str {
        self.src_url.as_str()
    }

    fn version(&self) -> &str {
        self.version.as_str()
    }

    fn commit(&self) -> &str {
        self.commit.as_str()
    }

    fn status(&self) -> ApplicationStatus {
        self.status.clone()
    }

    fn results_url(&self) -> &str {
        self.results_url.as_str()
    }

    fn deployment_url(&self) -> &str {
        self.deployment_url.as_str()
    }

    fn instances(&self) -> Vec<Option<ApplicationInstance>> {
        self.instances.clone()
    }
}

#[juniper::object(context = Database)]
#[graphql(description = "A physical devide which is a member of the platform")]
impl Platform {
    fn deployments(&self) -> Vec<Deployment> {
        self.deployments.clone()
    }

    fn orchestrator(&self) -> Orchestrator {
        self.orchestrator.clone()
    }

    fn nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }
}

pub struct Query;

#[juniper::object(Context = Database)]
impl Query {
    /// Get information for a specific deployment on the platform
    fn get_deployment(
        context: &Database,
        deployment_id: String,
    ) -> FieldResult<Option<&Deployment>> {
        let res = context.get_deployment(&deployment_id);
        // Return the result.
        Ok(res)
    }

    /// Get a list of all deployments running on the platform
    fn get_deployments(context: &Database) -> FieldResult<Option<Vec<Deployment>>> {
        let res = context.get_deployments();
        // Return the result.
        Ok(res)
    }

    /// Get a list of all services the orchestrator is providing to the platform
    fn get_platform_services(context: &Database) -> FieldResult<Option<Vec<Service>>> {
        let res = context.get_platform_services();
        // Return the result.
        Ok(res)
    }

    /// Get information for a specific node on the platform
    fn get_node(context: &Database, node_id: String) -> FieldResult<Option<&Node>> {
        //let res = context.db.lock().unwrap().get_node(&node_id);
        let res = context.get_node(&node_id);
        // Return the result.
        Ok(res)
    }

    /// Get a list of all nodes currently attached to the platform
    fn get_nodes(context: &Database) -> FieldResult<Option<Vec<Node>>> {
        //let res = context.db.lock().unwrap().get_node(&node_id);
        let res = context.get_nodes();
        // Return the result.
        Ok(res)
    }

    fn get_platform(context: &Database) -> FieldResult<Platform> {
        match (context.get_deployments(), context.get_nodes()) {
            (Some(deployments), Some(nodes)) => Ok(Platform::new(
                &deployments,
                &context.get_orchestrator(),
                &nodes,
            )),
            _ => Err(FieldError::new(
                "No Platform Available",
                juniper::graphql_value!({"internal_error": "no_platform_available"}),
            )),
        }
    }

    //fn get_platform(context: &Database) -> FieldResult<Option<Platform>> {
    //}
}
