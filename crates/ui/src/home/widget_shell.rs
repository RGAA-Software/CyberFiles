//! Shared Home widget chrome (card grid, icons, drive space bar).

use std::path::Path;

use cyberfiles_platform_windows::{icon_hint_for_path, ShellIconHint};
use gpui::{prelude::*, *};
use gpui_component::{progress::Progress, Icon, IconName, Sizable as _};

pub const CARD_WIDTH: Pixels = px(240.);
pub const CARD_MIN_HEIGHT: Pixels = px(72.);
pub const FOLDER_CARD_WIDTH: Pixels = px(120.);
pub const FOLDER_CARD_HEIGHT: Pixels = px(88.);

pub fn card_grid(children: impl IntoIterator<Item = AnyElement>) -> impl IntoElement {
    div()
        .id("home-card-grid")
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(children)
}

pub fn shell_icon_for_path(path: &Path) -> Icon {
    Icon::new(shell_icon_name(icon_hint_for_path(path))).small()
}

fn shell_icon_name(hint: ShellIconHint) -> IconName {
    match hint {
        ShellIconHint::Folder => IconName::Folder,
        ShellIconHint::Symlink => IconName::ExternalLink,
        ShellIconHint::Executable => IconName::Settings2,
        ShellIconHint::Image => IconName::File,
        ShellIconHint::Archive => IconName::Folder,
        ShellIconHint::File => IconName::File,
    }
}

pub fn space_progress_bar(id: impl Into<ElementId>, fraction: f32) -> impl IntoElement {
    Progress::new(id)
        .w_full()
        .h(px(4.))
        .value(fraction.clamp(0., 1.) * 100.)
}
