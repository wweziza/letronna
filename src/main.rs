#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod assets;
mod backend;
mod gateway;
mod prelude;
#[cfg(windows)]
mod setup;
mod store;
mod theme;
#[cfg(windows)]
mod tray;
mod ui;

use prelude::*;

fn main() {
    #[cfg(windows)]
    {
        if std::env::args().any(|a| a == "--uninstall") {
            setup::uninstall();
        }
        if setup::ensure_installed() || !setup::single_instance() {
            return;
        }
    }
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
        #[cfg(windows)]
        tray::init(cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let window = cx
            .open_window(
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
        #[cfg(windows)]
        let _ = window.update(cx, |_, window, cx| {
            window.on_window_should_close(cx, |window, _| {
                tray::hide(window);
                false
            });
        });
        #[cfg(not(windows))]
        let _ = window;
        cx.activate(true);
    });
}
