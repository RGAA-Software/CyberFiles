//! Embedded icon assets from the [Files](https://github.com/files-community/Files) app.
//!
//! Toolbar and navigation icons use Files `ThemedIcon` SVGs. Window chrome, theme toggle,
//! GitHub, and tab-close icons stay on gpui-component (Lucide) artwork.
//!
//! Run `python scripts/sync_files_icons.py` after updating `../Files` to refresh SVGs.

use gpui::{AssetSource, Result, SharedString};
use gpui_component_assets::Assets as ComponentAssets;
use std::borrow::Cow;
use std::sync::OnceLock;

/// GPUI icon paths that must use bundled Lucide SVGs, not Files ThemedIcon replacements.
const LUCIDE_ICON_PATHS: &[&str] = &[
    "icons/window-close.svg",
    "icons/window-minimize.svg",
    "icons/window-maximize.svg",
    "icons/window-restore.svg",
    "icons/github.svg",
    "icons/moon.svg",
    "icons/sun.svg",
    "icons/close.svg",
];

fn component_assets() -> &'static ComponentAssets {
    static ASSETS: OnceLock<ComponentAssets> = OnceLock::new();
    ASSETS.get_or_init(|| ComponentAssets::new(""))
}

fn use_lucide_icon(path: &str) -> bool {
    LUCIDE_ICON_PATHS.contains(&path)
}

/// GPUI icon paths used in CyberFiles mapped to Files `App.ThemedIcons.*` keys.
pub const ICON_MAP: &[(&str, &str)] = &[
    ("arrow-left", "NavBack"),
    ("arrow-right", "NavForward"),
    ("arrow-up", "NavUp"),
    ("redo-2", "Refresh"),
    ("panel-left-open", "PanelLeft"),
    ("panel-left-close", "PanelLeftClose"),
    ("panel-right-open", "PanelRight"),
    ("panel-right-close", "PanelRightClose"),
    ("panel-left", "PanelLeft"),
    ("layout-dashboard", "Settings.General.Widgets"),
    ("star", "Favorite"),
    ("plus", "New.Item"),
    ("folder", "Folder"),
    ("file", "File"),
    ("gallery-vertical-end", "FavoritePin"),
    ("delete", "Actions.Recycle"),
    ("chevron-right", "NavForward.12"),
    ("chevron-down", "NavForward.12"),
    ("external-link", "Shortcut"),
    ("settings-2", "Settings"),
    ("inbox", "Tag"),
    ("info", "Info"),
    ("bell", "StatusCenter"),
    ("hard-drive", "Actions.Eject"),
    ("globe", "Settings.General.Connections"),
    ("calendar", "Settings.General.TimeDate"),
];

/// Path under embedded assets for a Files ThemedIcon key (e.g. `NavBack` -> `icons/files/navback.svg`).
pub fn files_icon_path(key: &str) -> String {
    let file = key.replace('.', "_").to_lowercase();
    format!("icons/files/{file}.svg")
}

#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
#[include = "icons/**"]
#[include = "files-app/**"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if use_lucide_icon(path) {
            return component_assets().load(path);
        }

        if let Some(file) = Self::get(path) {
            return Ok(Some(file.data));
        }

        component_assets().load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names: Vec<SharedString> = Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();
        let mut from_component = component_assets().list(path)?;
        names.append(&mut from_component);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lucide_window_and_chrome_icons_load() {
        let assets = Assets;
        for path in LUCIDE_ICON_PATHS {
            let data = assets.load(path).expect("load");
            assert!(data.is_some(), "missing {path}");
        }
    }
}
