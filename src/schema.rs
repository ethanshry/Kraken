use crate::db::{Database, ManagedDatabase};
use crate::model::{Node, Service, ServiceStatus};
use juniper::FieldResult;

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

    fn current_cpu_percent(&self) -> f64 {
        self.current_cpu_percent()
    }

    fn current_ram_percent(&self) -> f64 {
        self.current_ram_percent()
    }

    fn current_disk_percent(&self) -> f64 {
        self.current_disk_percent()
    }

    fn application_instances(&self) -> bool {
        self.application_instances
    }

    fn services(&self, context: &Database) -> Option<&Vec<Service>> {
        //context.db.lock().unwrap().get_services(&self.id)
        context.get_services(&self.id)
    }
}

pub struct Query;

#[juniper::object(Context = Database)]
impl Query {
    fn get_services(context: &Database, node_id: String) -> FieldResult<Option<&Vec<Service>>> {
        //let res = context.db.lock().unwrap().get_services(&node_id);
        let res = context.get_services(&node_id);
        // Return the result.
        Ok(res)
    }

    fn get_node(context: &Database, node_id: String) -> FieldResult<Option<&Node>> {
        //let res = context.db.lock().unwrap().get_node(&node_id);
        let res = context.get_node(&node_id);
        // Return the result.
        Ok(res)
    }
}
