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
    uptime: u64,
    ram_free: u64,
    ram_used: u64,
    load_avg_5: f32,
    pub application_instances: bool,
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
        uptime: u64,
        ram_free: u64,
        ram_used: u64,
        load_avg_5: f32,
        application_instances: bool,
    ) -> Node {
        Node {
            id: id.to_owned(),
            model: model.to_owned(),
            uptime,
            ram_free,
            ram_used,
            load_avg_5,
            application_instances,
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

    pub fn update(&mut self, ram_free: &str, ram_used: &str, uptime: &str, load_avg_5: &str) -> () {
        self.ram_free = ram_free.parse::<u64>().unwrap();
        self.ram_used = ram_used.parse::<u64>().unwrap();
        self.uptime = uptime.parse::<u64>().unwrap();
        self.load_avg_5 = load_avg_5.parse::<f32>().unwrap();
    }

    pub fn from_msg(data: &Vec<String>) -> Node {
        Node {
            id: data.get(0).unwrap().clone(),
            model: "custom-pc".to_owned(),
            uptime: data.get(1).unwrap().parse::<u64>().unwrap(),
            ram_free: data.get(2).unwrap().parse::<u64>().unwrap(),
            ram_used: data.get(3).unwrap().parse::<u64>().unwrap(),
            load_avg_5: data.get(4).unwrap().parse::<f32>().unwrap(),
            application_instances: false,
            services: vec![],
        }
    }
}
