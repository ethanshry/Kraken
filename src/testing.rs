// Module specifically to setup the platform to run tests
// use crate::model::{ApplicationStatus, Deployment};
use crate::platform_executor::orchestrator::Orchestrator;
use crate::platform_executor::GenericNode;

// use uuid::Uuid;

pub async fn setup_experiment(_node: &mut GenericNode, _o: &mut Orchestrator) {
    || -> () {
        /*let arc = o.db_ref.clone();
        let mut db = arc.lock().unwrap();
        db.insert_deployment(&Deployment::new(
            &Uuid::new_v4().to_hyphenated().to_string(),
            "https://github.com/ethanshry/scapegoat",
            "",
            "",
            ApplicationStatus::DeploymentRequested,
            "",
            "",
            &vec![None],
        ));
        */
    }();
    /*
    let out =
        crate::gitapi::GitApi::get_tail_commits_for_repo_branches("ethanshry", "scapegoat").await;
    println!("{:?}", out);

    let out =
        crate::gitapi::GitApi::check_for_file_in_repo("ethanshry", "scapegoat", "shipwreck.toml")
            .await;
    println!("{:?}", out);
    */
}
