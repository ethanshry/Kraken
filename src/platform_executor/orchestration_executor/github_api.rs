//! Utility for working with the Github API

use async_std::fs::File;
use b64::FromBase64;
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

/// Gets the last commit hash for a specific github URL and branch name, if available
pub async fn get_tail_commit_for_branch_from_url(
    branch_name: &str,
    git_url: &str,
) -> Option<String> {
    match parse_git_url(git_url) {
        None => None,
        Some(url_data) => {
            match get_tail_commits_for_repo_branches(&url_data.user, &url_data.repo).await {
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
    if !url_parts[0].contains("github.com") {
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
/// # Examples
/// ```
/// assert_eq!(check_for_file_in_repo("ethanshry", "Kraken-UI", "shipwreck.toml"), None);
/// assert_eq!(check_for_file_in_repo("ethanshry", "scapenode", "shipwreck.toml"), Some(_));
/// ```
pub async fn check_for_file_in_repo(user: &str, repo: &str, file_path: &str) -> Option<String> {
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
                    // Shoutout https://stackoverflow.com/questions/40768678/decoding-base64-while-using-github-api-to-download-a-file
                    // TLDR Github uses MIME (RFC2045) encoding, which the b64 crate handles internally but the base64 crate does not support
                    let out = d
                        .content
                        .from_base64()
                        .expect("Failed to convert from base64");
                    let bytes = String::from_utf8(out).unwrap();
                    Some(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn check_for_file_in_repo_can_find_files() {
        let no_shipwreck_file_repo = Runtime::new()
            .expect("Failed to create tokio runtime")
            .block_on(check_for_file_in_repo(
                "ethanshry",
                "Kraken-UI",
                "shipwreck.toml",
            ));
        assert!(no_shipwreck_file_repo.is_none());
        let shipwreck_file_repo = Runtime::new()
            .expect("Failed to create tokio runtime")
            .block_on(check_for_file_in_repo(
                "ethanshry",
                "scapegoat",
                "shipwreck.toml",
            ));
        assert!(shipwreck_file_repo.is_some());
        println!("{:?}", shipwreck_file_repo);
    }
}
