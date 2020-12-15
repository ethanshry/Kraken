//! Misc utility functions and consts for the platform

use crate::gql_model::{ApplicationStatus, Orchestrator, OrchestratorInterface, Platform};
use crate::platform_executor::orchestration_executor::db::Database;
use std::fs;
use std::io::Write;
use uuid::Uuid;

pub const ROCKET_PORT_NO: u16 = 8000;
pub const UI_GIT_ADDR: &str = "https://github.com/ethanshry/Kraken-UI.git";

/// Pulls the systemId from the id.txt file, if one exists
pub fn get_system_id() -> String {
    return match fs::read_to_string("id.txt") {
        Ok(contents) => {
            let id = contents.parse::<String>().unwrap();
            std::env::set_var("SYSID", &id);
            return id;
        }
        Err(_) => {
            let mut file = fs::File::create("id.txt").unwrap();
            let contents = Uuid::new_v4();
            file.write_all(contents.to_hyphenated().to_string().as_bytes())
                .unwrap();
            contents.to_hyphenated().to_string()
        }
    };
}

/// Attempts to load a local platform configuration file if it exists, or creates a new one if it does not
pub fn load_or_create_platform(db: &mut Database) -> Platform {
    let platform = match fs::read_to_string("platform.json") {
        Ok(contents) => serde_json::from_str(&contents).unwrap(),
        Err(_) => Platform::new(
            &vec![],
            &Orchestrator::new(OrchestratorInterface::new(
                Some(String::from(UI_GIT_ADDR)),
                None,
                ApplicationStatus::Errored,
            )),
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

    platform
}
