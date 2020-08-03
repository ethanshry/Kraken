use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub git_username: String,
    pub repo_name: String,
    pub build_branch: String,
}
