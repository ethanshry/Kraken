use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Commit {
    pub sha: String,
    url: String,
}

#[derive(Deserialize, Debug)]
pub struct Branch {
    pub name: String,
    pub commit: Commit,
    protected: bool,
}

pub struct GitApi {}

impl GitApi {
    /// Grabs the most recent commits for all branches in a given repo
    pub fn get_tail_commits_for_repo_branches(user: &str, repo: &str) -> Option<Vec<Branch>> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/branches",
            owner = user,
            repo = repo
        );

        println!("Making request to: {}", url);

        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(USER_AGENT, "Kraken")
            .send();

        match response {
            Ok(r) => match r.json() {
                Ok(data) => data,
                Err(e) => {
                    println!("Failed to parse JSON: {}", e);
                    None
                }
            },
            Err(e) => {
                println!("Error in reqwest to {}: {}", url, e);
                None
            }
        }
    }
}
