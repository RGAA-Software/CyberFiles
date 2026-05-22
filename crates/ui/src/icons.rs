//! App-wide Material icon render sizes (SVG assets are 24×24; on-screen size is unified).

use gpui::{div, prelude::*, px, App};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};

const APP_ICON_PX: Size = Size::Size(px(18.));

fn sized_icon(icon: IconName) -> Icon {
    Icon::new(icon).with_size(APP_ICON_PX)
}

/// Toolbar, title bar, breadcrumbs, sidebar, settings, tab bar — all 18px.
pub fn toolbar_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

/// Icon tinted with the active theme primary text color (`currentColor` in SVG).
pub fn icon_foreground(icon: IconName, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .text_color(cx.theme().foreground)
        .child(toolbar_icon(icon))
}

/// Icon tinted for secondary / metadata text.
pub fn icon_muted(icon: IconName, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .text_color(cx.theme().muted_foreground)
        .child(toolbar_icon(icon))
}

pub fn sidebar_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

pub fn inline_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

pub fn compact_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

/// Pinned folder / push pin (`icons/pin.svg`, Material `push_pin`).
pub fn pin_icon() -> Icon {
    Icon::new(IconName::Folder)
        .path("icons/pin.svg")
        .with_size(APP_ICON_PX)
}
