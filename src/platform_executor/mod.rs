pub mod orchestrator;

use async_trait::async_trait;
use futures::Future;

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
        task: &'a F,
    ) -> ()
    where
        R: Future<Output = ()>,
        F: Send + Sync + Fn() -> R;
    async fn setup(&self) -> Result<(), SetupFaliure>;
}

pub struct Tester<'a, R: Future<Output = ()>, F: Send + Sync + Fn() -> R> {
    setup_tasks: Vec<&'a F>,
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
    fn add_setup_task(&mut self, task: &'a F) -> () {
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
