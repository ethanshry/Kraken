#[macro_use]
extern crate serde

#[derive(Deserialize, Debug)]
struct Commit {
    sha: String,
    url: String
}

#[derive(Deserialize, Debug)]
struct Branch {
    name: String
    commit: Commit,
    protected: bool
}

struct GitApi {}

impl GitApi {
    fn get_commits(user: &str, branch: &str) -> Vec<Commit> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/branches", owner = user, repo = branch)
        let mut response = reqwest::get(&url);
        response.json()?
    }
}