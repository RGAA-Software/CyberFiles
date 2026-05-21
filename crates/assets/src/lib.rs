//! Embedded icon assets from the [Files](https://github.com/files-community/Files) app.
//!
//! Run `python scripts/sync_files_icons.py` after updating `../Files` to refresh SVGs.

use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

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
    ("close", "Delete"),
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
    ("moon", "Settings.Appearance"),
    ("sun", "Settings.General.Theme"),
    ("github", "Settings.General.GitHub"),
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

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}
