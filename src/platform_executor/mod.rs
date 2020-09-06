pub mod orchestrator;

use async_trait::async_trait;
use futures::Future;
use std::pin::Pin;

pub enum SetupFaliure {
    NoPlatform, // Could not connect to existing platform
}

pub enum TaskFaliure {
    SigKill, // Task should be killed after this execution
}

#[async_trait]
trait ExecutionNode {
    fn add_setup_task(
        &mut self,
        task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>,
    ) -> ();
    async fn setup(&self) -> Result<(), SetupFaliure>;

    fn add_execution_task(
        &mut self,
        task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), TaskFaliure>>>,
    ) -> ();
    fn execute(&self) -> Result<(), ()>;
}

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

#[async_trait]
pub trait TesterTrait3 {
    fn add_setup_task<F>(
        &mut self,
        //task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>,
        //task: &'a (dyn Send + Sync + Fn() -> dyn Future<Output = ()>),
        task: F,
    ) -> ()
    where
        F: Unpin + Sync + Send + Fn() -> Pin<Box<dyn Send + Future<Output = ()>>> + 'static;
    async fn setup(&self) -> Result<(), SetupFaliure>;
}

pub struct Tester3 {
    setup_tasks:
        Vec<Box<dyn Unpin + Sync + Send + Fn() -> Pin<Box<dyn Send + Future<Output = ()>>>>>,
}

impl Tester3 {
    /// Creates a new Orchestrator Object
    pub fn new() -> Tester2 {
        Tester2 {
            setup_tasks: vec![],
        }
    }
}

#[async_trait]
impl TesterTrait3 for Tester3 {
    fn add_setup_task<F>(&mut self, task: F) -> ()
    where
        F: Unpin + Sync + Send + Fn() -> Pin<Box<dyn Send + Future<Output = ()>>> + 'static,
    {
        self.setup_tasks.push(Box::new(task))
    }
    async fn setup(&self) -> Result<(), SetupFaliure> {
        for task in self.setup_tasks.iter() {
            let t = task.clone();
            println!("EXECUTING");
            task().await;
            ()
        }
        Ok(())
    }
}
