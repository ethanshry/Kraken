use crate::platform_executor::{ExecutionNode, SetupFaliure, TaskFaliure};

use async_trait::async_trait;
use std::future::Future;

pub struct Orchestrator {
    setup_tasks: Vec<Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>>,
    execution_tasks: Vec<
        Box<dyn Send + Sync + Send + Sync + Fn() -> dyn Future<Output = Result<(), TaskFaliure>>>,
    >,
}

impl Orchestrator {
    /// Creates a new Orchestrator Object
    pub fn new() -> Orchestrator {
        Orchestrator {
            setup_tasks: vec![],
            execution_tasks: vec![],
        }
    }
}

#[async_trait]
impl ExecutionNode for Orchestrator {
    fn add_setup_task(
        &mut self,
        task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), SetupFaliure>>>,
    ) -> () {
        self.setup_tasks.push(task)
    }
    async fn setup(&self) -> Result<(), SetupFaliure> {
        //for task in self.setup_tasks {
        //(*task)().await;
        //}
        Ok(())
    }

    fn add_execution_task(
        &mut self,
        task: Box<dyn Send + Sync + Fn() -> dyn Future<Output = Result<(), TaskFaliure>>>,
    ) -> () {
        self.execution_tasks.push(task)
    }
    fn execute(&self) -> Result<(), ()> {
        Ok(())
    }
}
