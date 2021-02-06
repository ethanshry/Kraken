//! Specification and validation for the shipwreck.toml file
//!
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;

/// Defines the sections in the shipwreck.toml file in a requested deployment's root
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub app: AppConfig,
    pub config: DeploymentConfig,
}

/// Data required to configure the specifics of a deployment
#[derive(Serialize, Deserialize, Debug)]
pub struct DeploymentConfig {
    pub branch: String,
    pub lang: String,
    pub test: String,
    pub run: String,
    pub port: i64,
}

/// Metadata pertaining to the application (not required for the actual deployment)
#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    pub name: String,
    pub version: String,
    pub author: String,
    pub endpoint: String,
}

/// Ensure the required keys have been parsed and are valid
/// This will potentially include validation to ensure requested configs are platform-compliant (i.e. correct ports, etc)
/// TODO flush this out
fn validate_config(c: Option<Config>) -> Option<Config> {
    match c {
        Some(c) => Some(c),
        None => None,
    }
}

/// Parses a config file from a shipwreck.toml at the given path
pub fn get_config_for_path(p: &str) -> Option<Config> {
    match File::open(&p) {
        Err(_) => None,
        Ok(mut file) => {
            let mut file_data = String::new();
            file.read_to_string(&mut file_data).unwrap();

            get_config_for_string(&file_data)
        }
    }
}

/// Parses a config file from a shipwreck.toml from a string of data
pub fn get_config_for_string(data: &str) -> Option<Config> {
    let data: Config = toml::from_str(&data).unwrap();
    println!("{:?}", data);
    validate_config(Some(data))
}
