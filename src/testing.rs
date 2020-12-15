//! Module specifically to setup the platform to run tests

use crate::platform_executor::orchestration_executor::OrchestrationExecutor;
use crate::platform_executor::GenericNode;

pub async fn setup_experiment(_node: &mut GenericNode, _o: &mut OrchestrationExecutor) {}
