use crate::db::{Database, ManagedDatabase};
use crate::model::{ApplicationStatus, Deployment, Node, Orchestrator, Platform, Service};
use juniper::{FieldError, FieldResult};
use log::{error, info};
use uuid::Uuid;

impl juniper::Context for Database {}
impl juniper::Context for ManagedDatabase {}

// Specify computed resolvers for GQL type
// Note only visible resolvers can be here, other properties must be in a different impl block
#[juniper::object(context = ManagedDatabase)]
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

#[juniper::object(context = ManagedDatabase)]
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

    fn node(&self) -> &str {
        self.node.as_str()
    }
}

#[juniper::object(context = ManagedDatabase)]
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

#[juniper::object(Context = ManagedDatabase)]
impl Query {
    /// Get information for a specific deployment on the platform
    fn get_deployment(
        context: &ManagedDatabase,
        deployment_id: String,
    ) -> FieldResult<Option<Deployment>> {
        let db = context.db.lock().unwrap();
        Ok(db.get_deployment(&deployment_id))
    }

    /// Get a list of all deployments running on the platform
    fn get_deployments(context: &ManagedDatabase) -> FieldResult<Option<Vec<Deployment>>> {
        Ok(context.db.lock().unwrap().get_deployments())
    }

    /// Get a list of all services the orchestrator is providing to the platform
    fn get_platform_services(context: &ManagedDatabase) -> FieldResult<Option<Vec<Service>>> {
        Ok(context.db.lock().unwrap().get_platform_services())
    }

    /// Get information for a specific node on the platform
    fn get_node(context: &ManagedDatabase, node_id: String) -> FieldResult<Option<Node>> {
        //let res = context.db.lock().unwrap().get_node(&node_id);
        Ok(context.db.lock().unwrap().get_node(&node_id))
    }

    /// Get a list of all nodes currently attached to the platform
    fn get_nodes(context: &ManagedDatabase) -> FieldResult<Option<Vec<Node>>> {
        //let res = context.db.lock().unwrap().get_node(&node_id);
        info!("Getting nodes!");
        let res = context.db.lock().unwrap().get_nodes();
        // Return the result.
        Ok(res)
    }

    fn get_platform(context: &ManagedDatabase) -> FieldResult<Platform> {
        let db = context.db.lock().unwrap();
        match (db.get_deployments(), db.get_nodes()) {
            (Some(deployments), Some(nodes)) => {
                Ok(Platform::new(&deployments, &db.get_orchestrator(), &nodes))
            }
            _ => Err(FieldError::new(
                "No Platform Available",
                juniper::graphql_value!({"internal_error": "no_platform_available"}),
            )),
        }
    }

    //fn get_platform(context: &Database) -> FieldResult<Option<Platform>> {
    //}
}

pub struct Mutation;

#[juniper::object(Context = ManagedDatabase)]
impl Mutation {
    fn create_deployment(context: &ManagedDatabase, deployment_url: String) -> FieldResult<String> {
        let uuid = Uuid::new_v4().to_hyphenated().to_string();
        context
            .db
            .lock()
            .unwrap()
            .insert_deployment(&Deployment::new(
                &uuid,
                &deployment_url,
                "",
                "",
                ApplicationStatus::DeploymentRequested,
                "",
                "",
                "", //&vec![None],
            ));
        Ok(uuid)
    }

    fn poll_redeploy(context: &ManagedDatabase, deployment_id: String) -> FieldResult<bool> {
        let mut db = context.db.lock().unwrap();
        match db.get_deployment(&deployment_id) {
            Some(mut d) => {
                d.status = ApplicationStatus::UpdateRequested;
                db.update_deployment(&deployment_id, &d);
                Ok(true)
            }
            None => Err(FieldError::new(
                "No Active Deployment with the specified id",
                juniper::graphql_value!({"internal_error": "no_deployment_id"}),
            )),
        }
    }

    fn cancel_deployment(context: &ManagedDatabase, deployment_id: String) -> FieldResult<bool> {
        let mut db = context.db.lock().unwrap();
        match db.get_deployment(&deployment_id) {
            Some(mut d) => {
                d.status = ApplicationStatus::DestructionRequested;
                db.update_deployment(&deployment_id, &d);
                Ok(true)
            }
            None => Err(FieldError::new(
                "No Active Deployment with the specified id",
                juniper::graphql_value!({"internal_error": "no_deployment_id"}),
            )),
        }
    }

    //fn get_platform(context: &Database) -> FieldResult<Option<Platform>> {
    //}
}
