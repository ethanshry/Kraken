use log::{info, warn};
use std::fs;
use std::io;
use std::io::prelude::*;

/// Copies a directory's contents to crate/static/
/// Will persist subdirectory structure
pub fn copy_dir_contents_to_static(dir: &str) -> () {
    match fs::remove_dir_all("static") {
        Ok(_) => true,
        Err(_) => false,
    };
    fs::create_dir("static").unwrap();
    fn copy_dir_with_parent(root: &str, dir: &str) -> () {
        if root != "" {
            fs::create_dir(format!("static/{}", root)).unwrap();
        }
        info!("{} {}", root, dir);
        for item in fs::read_dir(dir).unwrap() {
            let path = &item.unwrap().path();
            if path.is_dir() {
                let folder_name = path.to_str().unwrap().split("/").last().unwrap();
                copy_dir_with_parent(
                    format!("{}/{}", root, folder_name).as_str(),
                    path.to_str().unwrap(),
                );
            } else {
                copy_file_to_static(root, &path.to_str().unwrap());
            }
        }
    }

    copy_dir_with_parent("", dir)
}

/// Copies an individual file to the crate/static directory
/// Leave file_path empty to coppy directly to the static directory
pub fn copy_file_to_static(target_subdir: &str, file_path: &str) -> Result<u64, std::io::Error> {
    let item_name = file_path.split("/").last().unwrap();
    match target_subdir {
        "" => fs::copy(file_path, format!("static/{}", item_name)),
        _ => fs::copy(file_path, format!("static/{}/{}", target_subdir, item_name)),
    }
}

/// Searches for a dockerfile and copies it to the target file path
/// File path should be the path to the repository root to copy the directory (not including the dockerfile name, which will be 'Dockerfile')
pub fn copy_dockerfile_to_dir(dockerfile_ref: &str, file_path: &str) -> bool {
    match fs::copy(
        format!("dockerfiles/{}", dockerfile_ref),
        format!("{}/Dockerfile", file_path),
    ) {
        Ok(_) => true,
        Err(e) => {
            info!(
                "There was an error copying the dockerfile {} to {}: {}",
                dockerfile_ref, file_path, e
            );
            false
        }
    }
}

/// Clears the crate's tmp directory if it exists.
/// Returns true if directory existed and was removed
/// Returns false if directory did not exist
pub fn clear_tmp() -> bool {
    match fs::remove_dir_all("tmp") {
        Ok(_) => true,
        Err(_) => false,
    }
}

pub fn append_to_file(file_path: &str, data: &str) {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path);

    match file {
        Ok(mut f) => {
            f.write_all(data.as_bytes());
        }
        Err(_) => {
            warn!("Error opening file at {}", file_path);
        }
    }
}

pub fn get_all_files_in_folder(path: &str) -> Result<Vec<String>, ()> {
    let entries = fs::read_dir(path);
    match entries {
        Ok(e) => {
            let results = e
                .map(|res| res.map(|e| format!("{}", e.path().display())))
                .collect::<Result<Vec<_>, io::Error>>()
                .unwrap_or(vec![]);
            Ok(results)
        }
        Err(_) => Err(()),
    }
}
