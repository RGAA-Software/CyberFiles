use cyberfiles_core::window_options;
use cyberfiles_ui::AppView;
use gpui::AppContext;
use gpui_component::Root;

fn main() {
    gpui_platform::application().run(move |cx| {
        gpui_component::init(cx);

        let window_options = window_options(cx);

        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| AppView);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open window");
        })
        .detach();
    });
}
