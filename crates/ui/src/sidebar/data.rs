use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cyberfiles_core::{AppConfig, FileTagConfig};
use cyberfiles_fs::{list_drives, DriveInfo};
#[cfg(windows)]
use cyberfiles_platform_windows::{
    list_cloud_drive_roots, list_known_folder_folders, list_shell_quick_access_folders,
    list_wsl_distro_roots, FOLDERID_LIBRARIES, FOLDERID_NETWORK,
};

use crate::shell::navigation::NavigationTarget;

use super::model::{SidebarEntry, SidebarSection, SidebarSectionKind};

pub fn build_sidebar_sections(config: &AppConfig) -> Vec<SidebarSection> {
    let mut sections = Vec::new();

    sections.push(SidebarSection {
        kind: SidebarSectionKind::Home,
        title: rust_i18n::t!("sidebar.section.main").to_string(),
        entries: vec![
            SidebarEntry {
                id: "home".into(),
                label: rust_i18n::t!("nav.home").to_string(),
                target: NavigationTarget::Home,
                pinned_in_settings: false,
            },
            SidebarEntry {
                id: "recycle".into(),
                label: rust_i18n::t!("nav.recycle_bin").to_string(),
                target: NavigationTarget::RecycleBin,
                pinned_in_settings: false,
            },
        ],
    });

    if config.show_sidebar_section_pinned {
        let entries = load_pinned_entries(config);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Pinned,
                title: rust_i18n::t!("sidebar.section.pinned").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_library {
        let entries = load_library_entries();
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Library,
                title: rust_i18n::t!("sidebar.section.library").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_drives {
        let entries = load_drive_entries();
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Drives,
                title: rust_i18n::t!("sidebar.section.drives").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_cloud {
        let entries = load_cloud_entries();
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Cloud,
                title: rust_i18n::t!("sidebar.section.cloud").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_network {
        let entries = load_network_entries();
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Network,
                title: rust_i18n::t!("sidebar.section.network").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_wsl {
        let entries = load_wsl_entries();
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Wsl,
                title: rust_i18n::t!("sidebar.section.wsl").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_file_tags {
        let entries = load_file_tag_entries(&config.file_tags);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::FileTags,
                title: rust_i18n::t!("sidebar.section.file_tags").to_string(),
                entries,
            });
        }
    }

    sections
}

fn load_pinned_entries(config: &AppConfig) -> Vec<SidebarEntry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    #[cfg(windows)]
    if let Ok(shell) = list_shell_quick_access_folders() {
        for item in shell {
            if item.path.exists() && seen.insert(path_key(&item.path)) {
                entries.push(SidebarEntry {
                    id: format!("pinned-shell-{}", entries.len()),
                    label: item.display_name,
                    target: NavigationTarget::Path(item.path),
                    pinned_in_settings: false,
                });
            }
        }
    }

    for path_str in &config.pinned_folders {
        let path = PathBuf::from(path_str);
        if !path.exists() || !seen.insert(path_key(&path)) {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        entries.push(SidebarEntry {
            id: format!("pinned-cfg-{path_str}"),
            label,
            target: NavigationTarget::Path(path),
            pinned_in_settings: true,
        });
    }

    entries
}

fn load_library_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_LIBRARIES)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.path.exists())
            .enumerate()
            .map(|(i, e)| SidebarEntry {
                id: format!("library-{i}"),
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_drive_entries() -> Vec<SidebarEntry> {
    list_drives()
        .into_iter()
        .map(|DriveInfo { path, label }| SidebarEntry {
            id: format!("drive-{}", label),
            label,
            target: NavigationTarget::Path(path),
            pinned_in_settings: false,
        })
        .collect()
}

fn load_cloud_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_cloud_drive_roots()
            .into_iter()
            .filter(|e| e.path.exists())
            .enumerate()
            .map(|(i, e)| SidebarEntry {
                id: format!("cloud-{i}"),
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_network_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_NETWORK)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.path.as_os_str().is_empty())
            .enumerate()
            .map(|(i, e)| SidebarEntry {
                id: format!("network-{i}"),
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_wsl_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_wsl_distro_roots()
            .into_iter()
            .filter(|e| e.path.exists())
            .enumerate()
            .map(|(i, e)| SidebarEntry {
                id: format!("wsl-{i}"),
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_file_tag_entries(tags: &[FileTagConfig]) -> Vec<SidebarEntry> {
    tags.iter()
        .enumerate()
        .filter(|(_, t)| !t.name.is_empty())
        .map(|(i, tag)| {
            let path = tag
                .paths
                .first()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .unwrap_or_else(home_fallback);
            SidebarEntry {
                id: format!("tag-{i}"),
                label: tag.name.clone(),
                target: NavigationTarget::Path(path),
                pinned_in_settings: false,
            }
        })
        .collect()
}

fn home_fallback() -> PathBuf {
    cyberfiles_fs::home_navigation_path()
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

