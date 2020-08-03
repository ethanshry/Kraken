use crate::db::Database;
use crate::model::{ApplicationStatus, Orchestrator, OrchestratorInterface, Platform};
use std::fs;
use std::io::{self, Write};
use uuid::Uuid;

pub fn get_system_id() -> String {
    return match fs::read_to_string("id.txt") {
        Ok(contents) => contents.parse::<String>().unwrap(),
        Err(_) => {
            let mut file = fs::File::create("id.txt").unwrap();
            let contents = Uuid::new_v4();
            file.write_all(contents.to_hyphenated().to_string().as_bytes())
                .unwrap();
            contents.to_hyphenated().to_string()
        }
    };
}

pub fn load_or_create_platform(db: &mut Database) -> Platform {
    let platform = match fs::read_to_string("platform.json") {
        Ok(contents) => serde_json::from_str(&contents).unwrap(),
        Err(_) => Platform::new(
            &vec![],
            &Orchestrator::new(
                OrchestratorInterface::new(
                    Some(String::from("https://github.com/ethanshry/Kraken-UI.git")),
                    None,
                    ApplicationStatus::ERRORED,
                )
                .to_owned(),
            ),
            &vec![],
        ),
    };

    db.update_orchestrator_interface(&platform.orchestrator.ui);

    for node in &platform.nodes {
        db.insert_node(&node);
    }

    for deployment in &platform.deployments {
        db.insert_deployment(&deployment);
    }

    return platform;
}
