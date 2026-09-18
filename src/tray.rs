use crate::prelude::*;
use std::time::Duration;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const RELEASES: &str = "https://github.com/wweziza/letronna/releases";
const UPDATES: &str = "check-updates";
const QUIT: &str = "quit";

struct Tray(#[allow(dead_code)] TrayIcon);
impl Global for Tray {}

pub(crate) fn init(cx: &mut App) {
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &MenuItem::new(BRAND, false, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(UPDATES, "Check for updates", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(QUIT, format!("Quit {BRAND}"), true, None),
    ]);
    let Ok(icon) = Icon::from_resource(1, None) else {
        return;
    };
    let Ok(tray) = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip(BRAND)
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()
    else {
        return;
    };
    cx.set_global(Tray(tray));

    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                let _ = cx.update(|cx| match event.id.0.as_str() {
                    UPDATES => cx.open_url(RELEASES),
                    QUIT => cx.quit(),
                    _ => {}
                });
            }
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } = event
                {
                    let _ = cx.update(show_main_window);
                }
            }
        }
    })
    .detach();
}

fn show_main_window(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| {
            set_visible(window, true);
            window.activate_window();
        });
    }
}

pub(crate) fn hide(window: &mut Window) {
    set_visible(window, false);
}

fn set_visible(window: &Window, visible: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd = h.hwnd.get() as *mut core::ffi::c_void;
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
        if visible {
            SetForegroundWindow(hwnd);
        }
    }
}
