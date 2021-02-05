//! Utilities interfacing with misc cli tools

use log::{warn};

/// Gets user-friendly node name
pub fn get_node_name() -> String {
    let get_name = || -> Result<String, String> {
        let mut res = std::process::Command::new("uname")
        .arg("-n")
        .spawn().expect("err");
        let name = res.wait().expect("err");
        Ok(format!("{}", name))
    };
    match get_name() {
        Ok(n) => n,
        _ => {
            warn!("Unable to find node name, using default");
            String::from("unknown-model")
        }
    }
}