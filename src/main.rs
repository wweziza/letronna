#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod assets;
mod core;
#[cfg(windows)]
mod platform;
mod plugins;
mod prelude;
mod theme;
mod ui;

use prelude::*;

fn main() {
    #[cfg(windows)]
    {
        if std::env::args().any(|a| a == "--uninstall") {
            platform::install::uninstall();
        }
        if platform::install::ensure_installed() || !platform::install::single_instance() {
            return;
        }
    }
    Application::new().with_assets(Assets).run(|cx| {
        gpui_component::init(cx);
        theme::apply(cx);
        plugins::init(
            cx,
            vec![
                Box::new(plugins::discord::DiscordPresence::new(
                    plugins::PresenceMode::Detailed,
                )),
                Box::new(plugins::hello::Hello::new()),
            ],
        );
        cx.bind_keys([
            KeyBinding::new("ctrl-n", NewSession, None),
            KeyBinding::new(
                "shift-enter",
                gpui_component::input::Enter { secondary: true },
                Some("Input"),
            ),
        ]);
        #[cfg(windows)]
        platform::tray::init(cx);
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
                platform::window::hide(window);
                false
            });
        });
        #[cfg(not(windows))]
        let _ = window;
        cx.activate(true);
    });
}
