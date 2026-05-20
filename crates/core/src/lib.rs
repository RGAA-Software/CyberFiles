use gpui::{size, px, App, WindowBounds, WindowOptions};

pub const APP_NAME: &str = "CyberFiles";

pub const WINDOW_WIDTH: f32 = 1280.;
pub const WINDOW_HEIGHT: f32 = 720.;

pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
            cx,
        )),
        ..Default::default()
    }
}
