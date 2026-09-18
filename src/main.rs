#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod assets;
mod backend;
mod gateway;
mod prelude;
mod store;
mod theme;
mod ui;

use prelude::*;

fn main() {
    Application::new().with_assets(Assets).run(|cx| {
        gpui_component::init(cx);
        theme::apply(cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-n", NewSession, None),
            KeyBinding::new(
                "shift-enter",
                gpui_component::input::Enter { secondary: true },
                Some("Input"),
            ),
        ]);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(760.)),
                    cx,
                ))),
                window_min_size: Some(size(px(720.), px(520.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(BRAND.into()),
                    ..TitleBar::title_bar_options()
                }),
                ..Default::default()
            },
            |window, cx| {
                let chat = cx.new(|cx| Chat::new(window, cx));
                cx.new(|cx| Root::new(chat, window, cx))
            },
        )
        .expect("Could not open native application window");
        cx.activate(true);
    });
}
