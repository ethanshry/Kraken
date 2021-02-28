//! Module specifically to provide baseline configuration the platform, typically used to perform testing

use crate::platform_executor::orchestration_executor::OrchestrationExecutor;
use crate::platform_executor::GenericNode;

pub async fn setup_experiment(_node: &mut GenericNode, _o: &mut OrchestrationExecutor) {}
