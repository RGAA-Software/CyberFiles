//! Shell context menu row icons (PNG from Windows menu bitmaps).

use std::sync::Arc;

use gpui::{img, prelude::*, Image, ImageFormat, ObjectFit, Pixels, Window, px};

/// Renders a 16×16 Shell menu icon from PNG bytes.
pub fn shell_menu_icon_img(png: Arc<Vec<u8>>, window: &Window) -> impl IntoElement {
    let size = px(16.) * window.scale_factor();
    img(Arc::new(Image::from_bytes(ImageFormat::Png, (*png).clone())))
        .size(Pixels::from(size))
        .object_fit(ObjectFit::Contain)
}
