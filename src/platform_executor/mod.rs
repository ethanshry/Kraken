pub mod orchestrator;

use crate::rabbit::RabbitBroker;
use async_trait::async_trait;

pub enum SetupFaliure {
    NoPlatform, // Could not connect to existing platform
}

pub enum TaskFaliure {
    SigKill, // Task should be killed after this execution
}

pub struct DeploymentTask {
    task: tokio::task::JoinHandle<()>,
    id: String,
}

pub struct QueueTask {
    task: tokio::task::JoinHandle<()>,
    queue_label: String,
}

pub struct GenericNode {
    pub deployment_processes: Vec<DeploymentTask>,
    pub queue_consumers: Vec<QueueTask>,
    pub system_id: String,   // TODO common
    pub rabbit_addr: String, // TODO common
}

impl GenericNode {
    pub fn new(system_id: &str, rabbit_addr: &str) -> GenericNode {
        GenericNode {
            deployment_processes: vec![],
            queue_consumers: vec![],
            rabbit_addr: rabbit_addr.to_owned(),
            system_id: system_id.to_owned(),
        }
    }
}

#[async_trait]
trait NodeUtils {
    async fn connect_to_rabbit_instance(&self) -> RabbitBroker;
}

#[async_trait]
impl NodeUtils for GenericNode {
    async fn connect_to_rabbit_instance(&self) -> RabbitBroker {
        match RabbitBroker::new(&self.rabbit_addr).await {
            Some(b) => b,
            None => panic!("Could not establish rabbit connection"),
        }
    }
}

#[async_trait]
trait ExecutionNode {
    async fn setup(&self) -> Result<(), SetupFaliure>;
    async fn execute(&self) -> Result<(), TaskFaliure>;
}

/*
#[async_trait]
pub trait TesterTrait<'a, R, F> {
    fn add_setup_task(
        &mut self,
        //task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>,
        //task: &'a (dyn Send + Sync + Fn() -> dyn Future<Output = ()>),
        task: &'a Box<F>,
    ) -> ()
    where
        R: Future<Output = ()>,
        F: Send + Sync + Fn() -> R;
    async fn setup(&self) -> Result<(), SetupFaliure>;
}

pub struct Tester<'a, R: Future<Output = ()>, F: Send + Sync + Fn() -> R> {
    setup_tasks: Vec<&'a Box<F>>,
}

impl<'a, R: Future<Output = ()>, F: Send + Sync + Fn() -> R> Tester<'a, R, F> {
    /// Creates a new Orchestrator Object
    pub fn new() -> Tester<'a, R, F> {
        Tester {
            setup_tasks: vec![],
        }
    }
}

#[async_trait]
impl<'a, R: Send + Sync + Future<Output = ()>, F: Send + Sync + Fn() -> R> TesterTrait<'a, R, F>
    for Tester<'a, R, F>
{
    fn add_setup_task(&mut self, task: &'a Box<F>) -> () {
        self.setup_tasks.push(task)
    }
    async fn setup(&self) -> Result<(), SetupFaliure> {
        for task in self.setup_tasks.iter() {
            task().await;
            ()
        }
        Ok(())
    }
}

#[async_trait]
pub trait TesterTrait2 {
    fn add_setup_task<F>(
        &mut self,
        //task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>,
        //task: &'a (dyn Send + Sync + Fn() -> dyn Future<Output = ()>),
        task: F,
    ) -> ()
    where
        F: Send + Sync + Fn() + 'static;
    async fn setup(&self) -> Result<(), SetupFaliure>;
}

pub struct Tester2 {
    setup_tasks: Vec<Box<dyn Send + Sync + Fn()>>,
}

impl Tester2 {
    /// Creates a new Orchestrator Object
    pub fn new() -> Tester2 {
        Tester2 {
            setup_tasks: vec![],
        }
    }
}

#[async_trait]
impl TesterTrait2 for Tester2 {
    fn add_setup_task<F>(&mut self, task: F) -> ()
    where
        F: Send + Sync + Fn() + 'static,
    {
        self.setup_tasks.push(Box::new(task))
    }
    async fn setup(&self) -> Result<(), SetupFaliure> {
        for task in self.setup_tasks.iter() {
            println!("EXECUTING");
            task();
            ()
        }
        Ok(())
    }
}
*/
