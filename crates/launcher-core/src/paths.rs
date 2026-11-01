use std::path::PathBuf;
use directories::ProjectDirs;

pub fn get_launcher_root_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "eneslauncher", "EnesLauncher") {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".eneslauncher")
    }
}

pub fn get_instances_dir() -> PathBuf {
    get_launcher_root_dir().join("instances")
}

pub fn get_shared_cache_dir() -> PathBuf {
    get_launcher_root_dir().join("shared")
}

pub fn get_staging_dir() -> PathBuf {
    get_launcher_root_dir().join("staging")
}

pub fn get_backups_dir() -> PathBuf {
    get_launcher_root_dir().join("backups")
}

