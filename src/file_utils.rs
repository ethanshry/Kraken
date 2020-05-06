use std::fs;

/// Copies a directory's contents to crate/static/
/// Will persist subdirectory structure
pub fn copy_dir_contents_to_static(dir: &str) -> () {
    fn copy_dir_with_parent(root: &str, dir: &str) -> () {
        if root != "" {
            fs::create_dir(format!("static/{}", root));
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
    };
}
