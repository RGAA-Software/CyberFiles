use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub path: PathBuf,
    pub label: String,
}

/// Lists ready local drive roots (e.g. `C:\`, `D:\`).
pub fn list_drives() -> Vec<DriveInfo> {
    #[cfg(windows)]
    {
        list_windows_drives()
    }

    #[cfg(not(windows))]
    {
        vec![DriveInfo {
            path: PathBuf::from("/"),
            label: "Root".to_string(),
        }]
    }
}

#[cfg(windows)]
fn list_windows_drives() -> Vec<DriveInfo> {
    let mut drives = Vec::new();

    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        let path = PathBuf::from(&root);
        if path.exists() {
            drives.push(DriveInfo {
                label: root.clone(),
                path,
            });
        }
    }

    drives
}

pub fn default_user_profile() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from).filter(|p| p.exists())
}

pub fn home_navigation_path() -> PathBuf {
    default_user_profile()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}
