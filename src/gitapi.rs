use log::info;
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

#[derive(Deserialize, Debug, Clone)]
pub struct FileData {
    pub name: String,
    pub sha: String,
    pub content: String,
}

pub struct GitUrl {
    pub user: String,
    pub repo: String,
}

pub struct GitApi {}

impl GitApi {
    pub async fn get_tail_commit_for_branch_from_url(
        branch_name: &str,
        git_url: &str,
    ) -> Option<String> {
        match GitApi::parse_git_url(git_url) {
            None => None,
            Some(url_data) => {
                match GitApi::get_tail_commits_for_repo_branches(&url_data.user, &url_data.repo)
                    .await
                {
                    None => None,
                    Some(items) => {
                        for i in items {
                            if i.name == branch_name {
                                return Some(i.commit.sha);
                            }
                        }
                        None
                    }
                }
            }
        }
    }

    /// Pulls username and reponame from a canidate git url
    pub fn parse_git_url(git_url: &str) -> Option<GitUrl> {
        // https://github.com/ethanshry/scapegoat
        let url_vec = git_url.split('/').collect::<Vec<&str>>();
        let mut url_parts = &url_vec[..];
        if url_parts.len() == 5 {
            url_parts = &url_parts[2..];
        }
        if url_parts.len() != 3 {
            return None;
        }
        Some(GitUrl {
            user: url_parts[1].to_string(),
            repo: url_parts[2].to_string(),
        })
    }

    /// Grabs the most recent commits for all branches in a given repo
    pub async fn get_tail_commits_for_repo_branches(user: &str, repo: &str) -> Option<Vec<Branch>> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/branches",
            owner = user,
            repo = repo
        );

        info!("Making request to: {}", url);

        let client = reqwest::Client::new();

        let response = client
            .get(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(USER_AGENT, "Kraken")
            .send()
            .await;

        match response {
            Ok(r) => match r.json().await {
                Ok(data) => data,
                Err(e) => {
                    info!("Failed to parse JSON: {}", e);
                    None
                }
            },
            Err(e) => {
                info!("Error in reqwest to {}: {}", url, e);
                None
            }
        }
    }

    /// Checks if a file exists in a given repo
    /// If the file exists, its FileData will be returned
    /// FileData currently is base64 encoded
    /// If the file does not exist, then None
    pub async fn check_for_file_in_repo(
        user: &str,
        repo: &str,
        file_path: &str,
    ) -> Option<FileData> {
        let url = format!(
            "https://api.github.com/repos/{owner}/{repo}/contents/{file}",
            owner = user,
            repo = repo,
            file = file_path
        );

        info!("Making request to: {}", url);

        let client = reqwest::Client::new();

        let response = client
            .get(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(USER_AGENT, "Kraken")
            .send()
            .await;

        match response {
            Ok(r) => {
                let data: Result<FileData, _> = r.json().await;
                match data {
                    Ok(d) => {
                        // TODO the base64::decode crashes, which isn't great
                        //let mut out: FileData = d.clone();
                        //let bytes = base64::decode(&d.content).unwrap();
                        //out.content = String::from_utf8(bytes).unwrap();
                        Some(d)
                    }
                    Err(e) => {
                        info!("Failed to parse JSON: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                info!("Error in reqwest to {}: {}", url, e);
                None
            }
        }
    }
}
