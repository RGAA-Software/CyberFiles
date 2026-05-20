use cyberfiles_core::{APP_NAME, window_size};
use gpui::{
    App, AppContext, Bounds, Focusable, SharedString, Size, Window, WindowBounds, WindowKind,
    WindowOptions, px, size,
};
use gpui_component::{Root, TitleBar};

use super::app_shell::AppShell;

pub fn open_main_window<F, E>(crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    let (width, height) = window_size();
    let mut window_size = size(px(width), px(height));
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = window_size.width.min(display_size.width * 0.85);
        window_size.height = window_size.height.min(display_size.height * 0.85);
    }
    let window_bounds = Bounds::centered(None, window_size, cx);
    let title = SharedString::from(APP_NAME);

    cx.spawn(async move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(Size {
                width: px(480.),
                height: px(320.),
            }),
            kind: WindowKind::Normal,
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };

        let window = cx
            .open_window(options, |window, cx| {
                let view = crate_view_fn(window, cx);
                let shell = cx.new(|cx| AppShell::new(title.clone(), view, window, cx));

                let focus_handle = shell.focus_handle(cx);
                window.defer(cx, move |window, cx| {
                    if window.focused(cx).is_none() {
                        focus_handle.focus(window, cx);
                    }
                });

                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("failed to open window");

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
