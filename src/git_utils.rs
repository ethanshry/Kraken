//! Wrappers for git CLI functions

/// Clones a git repository at a specific branch to the dst_dir
pub fn clone_remote_branch(url: &str, branch: &str, dst_dir: &str) -> std::process::Child {
    std::process::Command::new("git")
        .arg("clone")
        .arg("-b")
        .arg(branch)
        .arg(url)
        .arg(dst_dir)
        .spawn()
        .expect("err in clone")
}
