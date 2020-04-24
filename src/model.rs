use juniper;

#[derive(Clone, juniper::GraphQLEnum)]
pub enum ServiceStatus {
    OK,
    ERRORED,
}

// Specify GraphQL type with field resolvers (i.e. no computed resolvers)
#[derive(Clone, juniper::GraphQLObject)]
#[graphql(description = "A Service installed on the device to support the platform")]
pub struct Service {
    name: String,
    version: String,
    status: ServiceStatus,
}

impl Service {
    pub fn new(name: &str, version: &str, status: ServiceStatus) -> Service {
        Service {
            name: name.to_owned(),
            version: version.to_owned(),
            status,
        }
    }
}

//#[derive(juniper::GraphQLObject)]

pub struct Node {
    pub id: String,
    pub model: String,
    current_cpu: f32,
    current_ram: u64,
    current_disk: u64,
    max_cpu: f32,
    max_ram: u64,
    max_disk: u64,
    pub application_instances: bool,
    services: Vec<Service>,
}

impl Clone for Node {
    fn clone(&self) -> Self {
        let mut node = Node {
            id: self.id.to_owned(),
            model: self.model.to_owned(),
            current_cpu: self.current_cpu,
            current_disk: self.current_disk,
            current_ram: self.current_ram,
            max_cpu: self.max_cpu,
            max_ram: self.max_ram,
            max_disk: self.max_disk,
            application_instances: self.application_instances,
            services: Vec::new(),
        };
        for service in self.services.iter() {
            node.services.push(service.clone());
        }

        node
    }
}

// non-gql impl block for Node
impl Node {
    pub fn new(
        id: &str,
        model: &str,
        current_cpu: f32,
        current_disk: u64,
        current_ram: u64,
        max_cpu: f32,
        max_ram: u64,
        max_disk: u64,
        application_instances: bool,
    ) -> Node {
        Node {
            id: id.to_owned(),
            model: model.to_owned(),
            current_cpu,
            current_disk,
            current_ram,
            max_cpu,
            max_ram,
            max_disk,
            application_instances,
            services: vec![],
        }
    }

    pub fn current_cpu_percent(&self) -> f64 {
        (self.current_cpu / self.max_cpu) as f64
    }

    pub fn current_ram_percent(&self) -> f64 {
        (self.current_ram / self.max_ram) as f64
    }

    pub fn current_disk_percent(&self) -> f64 {
        (self.current_disk / self.max_disk) as f64
    }

    pub fn set_disk_ram_cpu(&mut self, cpu: f32, ram: u64, disk: u64) -> () {
        self.current_cpu = cpu;
        self.current_disk = disk;
        self.current_ram = ram;
    }
}
