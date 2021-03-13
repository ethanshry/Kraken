//! Specification and validation for the shipwreck.toml file
//!
//! The shipwreck.toml is the configuration file used by the
//! platform to determine how to deploy your app
//!
//! Example:
//! ```toml
//! [app]
//! name="scapenode"
//! version="1.0.0"
//! author="Ethan Shry"
//! endpoint="https://github.com/ethanshry/scapenode"
//!
//! [config]
//! port=7211
//! lang="node"
//! test="npm test"
//! run="npm start"
//! ```
//! This file should go in the root of your repository
//! The Kraken UI and Platform will look for it to validate
//! any deployment request it recieves
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
/// ```
/// let mut valid_config = get_config_for_path(
///     &format!(
///         "{}/test_assets/shipwreck.toml",
///         env!("CARGO_MANIFEST_DIR")
///     )
/// );
/// assert!(valid_config.is_some());
/// ```
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
    let data: Option<Config> = match toml::from_str(&data) {
        Ok(data) => Some(data),
        Err(_) => None,
    };
    validate_config(data)
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn configs_from_strings_validate() {
        let valid_config_string = "[app]\nname=\"scapenode\"\nversion=\"1.0.0\"\nauthor=\"Ethan Shry\"\nendpoint=\"https://github.com/ethanshry/scapenode\"\n\n[config]\nport=7211\nlang=\"node\"\ntest=\"npm test\"\nrun=\"npm start\"\n";

        let invalid_config_string = "hello world";

        let mut valid_config = get_config_for_string(valid_config_string);

        let mut invalid_config = get_config_for_string(invalid_config_string);

        assert!(valid_config.is_some());
        assert!(invalid_config.is_none());

        valid_config = validate_config(valid_config);
        invalid_config = validate_config(invalid_config);

        assert!(valid_config.is_some());
        assert!(invalid_config.is_none());
    }

    #[test]
    fn configs_from_files_validate() {
        let mut valid_config = get_config_for_path(&format!(
            "{}/test_assets/shipwreck.toml",
            env!("CARGO_MANIFEST_DIR")
        ));

        let mut invalid_config = get_config_for_path(&format!(
            "{}/test_assets/bad_shipwreck.toml",
            env!("CARGO_MANIFEST_DIR")
        ));

        assert!(valid_config.is_some());
        assert!(invalid_config.is_none());

        valid_config = validate_config(valid_config);
        invalid_config = validate_config(invalid_config);

        assert!(valid_config.is_some());
        assert!(invalid_config.is_none());
    }
}
