use std::path::PathBuf;

use cyberfiles_core::path_history_list;
use cyberfiles_fs::omnibar_path_suggestions;
use gpui::SharedString;

use crate::shell::navigation::NavigationTarget;

/// Omnibar mode (Files: path / command palette / search).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OmnibarMode {
    #[default]
    Path,
    CommandPalette,
}

#[derive(Debug, Clone)]
pub enum OmnibarSuggestion {
    Path {
        path: PathBuf,
        label: String,
    },
    Command {
        id: OmnibarCommand,
        label: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmnibarCommand {
    NavigateHome,
    OpenSettings,
    OpenRecycleBin,
    ToggleDualPane,
    ToggleInfoPane,
    NewTab,
}

impl OmnibarCommand {
    pub fn label(self) -> &'static str {
        match self {
            Self::NavigateHome => "Go to Home",
            Self::OpenSettings => "Open Settings",
            Self::OpenRecycleBin => "Open Recycle Bin",
            Self::ToggleDualPane => "Toggle dual pane",
            Self::ToggleInfoPane => "Toggle info pane",
            Self::NewTab => "New tab",
        }
    }

    pub fn all() -> &'static [OmnibarCommand] {
        &[
            Self::NavigateHome,
            Self::OpenSettings,
            Self::OpenRecycleBin,
            Self::ToggleDualPane,
            Self::ToggleInfoPane,
            Self::NewTab,
        ]
    }
}

pub fn refresh_suggestions(mode: OmnibarMode, query: &str) -> Vec<OmnibarSuggestion> {
    match mode {
        OmnibarMode::Path => omnibar_path_suggestions(query, &path_history_list())
            .into_iter()
            .map(|s| OmnibarSuggestion::Path {
                path: s.path,
                label: s.label,
            })
            .collect(),
        OmnibarMode::CommandPalette => filter_commands(query),
    }
}

fn filter_commands(query: &str) -> Vec<OmnibarSuggestion> {
    let needle = query.trim().to_ascii_lowercase();
    OmnibarCommand::all()
        .iter()
        .filter(|cmd| {
            needle.is_empty() || cmd.label().to_ascii_lowercase().contains(&needle)
        })
        .take(10)
        .map(|cmd| OmnibarSuggestion::Command {
            id: *cmd,
            label: cmd.label().to_string(),
        })
        .collect()
}

/// Resolves omnibar submit text to a navigation target when in path mode.
pub fn resolve_path_submit(text: &str) -> Option<NavigationTarget> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("home") {
        return Some(NavigationTarget::Home);
    }
    if trimmed.eq_ignore_ascii_case("settings") {
        return Some(NavigationTarget::Settings);
    }
    if trimmed.eq_ignore_ascii_case("recycle bin") || trimmed.eq_ignore_ascii_case("recycle") {
        return Some(NavigationTarget::RecycleBin);
    }

    let path = PathBuf::from(trimmed);
    if path.is_dir() {
        return Some(NavigationTarget::Path(path));
    }
    if path.is_file() {
        return path
            .parent()
            .map(|parent| NavigationTarget::Path(parent.to_path_buf()));
    }
    None
}

pub fn mode_button_label(mode: OmnibarMode) -> SharedString {
    SharedString::from(match mode {
        OmnibarMode::Path => "Path",
        OmnibarMode::CommandPalette => "Commands",
    })
}

pub fn mode_placeholder(mode: OmnibarMode) -> SharedString {
    SharedString::from(match mode {
        OmnibarMode::Path => rust_i18n::t!("nav.path.placeholder"),
        OmnibarMode::CommandPalette => rust_i18n::t!("omnibar.command.placeholder"),
    })
}
