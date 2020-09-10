// Module specifically to setup the platform to run tests
use crate::model::{ApplicationStatus, Deployment};
use crate::platform_executor::orchestrator::Orchestrator;
use crate::platform_executor::GenericNode;

pub async fn setup_experiment(_node: &mut GenericNode, o: &mut Orchestrator) {
    || -> () {
        let arc = o.db_ref.clone();
        let mut db = arc.lock().unwrap();
        db.insert_deployment(&Deployment::new(
            "d2697ce0-bb46-4622-bb2a-0340aae62514",
            "https://github.com/ethanshry/scapegoat",
            "",
            "",
            ApplicationStatus::REQUESTED,
            "",
            "",
            &vec![None],
        ))
        .unwrap();
    }();
}
