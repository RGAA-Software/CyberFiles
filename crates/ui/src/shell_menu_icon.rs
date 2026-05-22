//! Shell context menu row icons (PNG from Windows menu bitmaps).

use std::sync::Arc;

use gpui::{img, prelude::*, Image, ImageFormat, ObjectFit, Pixels, Window, px};

/// Logical on-screen size for Shell menu row icons (GPUI scales for HiDPI).
const SHELL_MENU_ICON_LOGICAL: Pixels = px(16.);

/// Renders a DPI-aware Shell menu icon inside a fixed 16×16 logical slot.
pub fn shell_menu_icon_img(png: Arc<Vec<u8>>, _window: &Window) -> impl IntoElement {
    img(Arc::new(Image::from_bytes(ImageFormat::Png, (*png).clone())))
        .size(SHELL_MENU_ICON_LOGICAL)
        .flex_none()
        .object_fit(ObjectFit::Contain)
}
