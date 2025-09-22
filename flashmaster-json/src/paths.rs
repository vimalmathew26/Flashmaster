use directories::ProjectDirs;
use std::path::PathBuf;

pub fn data_root() -> PathBuf {
    // org = "flashmaster", app = "FlashMaster"
    if let Some(pd) = ProjectDirs::from("com", "flashmaster", "FlashMaster") {
        pd.data_dir().to_path_buf()
    } else {
        // Fallback: current dir
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

pub fn default_store_file() -> (PathBuf, PathBuf) {
    let root = data_root();
    let file = root.join("flashmaster.json");
    let backups = root.join("backups");
    (file, backups)
}
