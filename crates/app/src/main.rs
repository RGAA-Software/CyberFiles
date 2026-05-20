use cyberfiles_ui::{Assets, MainPage, init, open_main_window};

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        open_main_window(
            move |window, cx| MainPage::view(window, cx),
            cx,
        );
    });
}
