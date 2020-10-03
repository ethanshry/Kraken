use log::info;
use std::fs;

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
pub fn copy_file_to_static(target_subdir: &str, file_path: &str) -> () {
    let item_name = file_path.split("/").last().unwrap();
    match target_subdir {
        "" => fs::copy(file_path, format!("static/{}", item_name)),
        _ => fs::copy(file_path, format!("static/{}/{}", target_subdir, item_name)),
    }
    .unwrap();
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
