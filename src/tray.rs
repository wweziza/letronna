//! Notification-area icon (Windows). Right-click opens a themed popup window
//! instead of the native context menu so it matches the app.

use crate::prelude::*;
use std::time::Duration;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const RELEASES: &str = "https://github.com/wweziza/letronna/releases";
const MENU_SIZE: Size<Pixels> = size(px(208.), px(112.));

struct Tray(#[allow(dead_code)] TrayIcon);
impl Global for Tray {}

pub(crate) fn init(cx: &mut App) {
    let Ok(icon) = Icon::from_resource(1, None) else {
        return;
    };
    let Ok(tray) = TrayIconBuilder::new()
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
                .timer(Duration::from_millis(100))
                .await;
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                let TrayIconEvent::Click {
                    button,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } = event
                else {
                    continue;
                };
                let _ = cx.update(|cx| match button {
                    MouseButton::Left => show_main_window(cx),
                    MouseButton::Right => open_menu(position.x as f32, position.y as f32, cx),
                    MouseButton::Middle => {}
                });
            }
        }
    })
    .detach();
}

fn main_window(cx: &App) -> Option<AnyWindowHandle> {
    cx.windows().into_iter().next()
}

fn show_main_window(cx: &mut App) {
    if let Some(handle) = main_window(cx) {
        let _ = handle.update(cx, |_, window, _| {
            set_visible(window, true);
            window.activate_window();
        });
    }
}

fn open_menu(x: f32, y: f32, cx: &mut App) {
    let scale = main_window(cx)
        .and_then(|h| h.update(cx, |_, window, _| window.scale_factor()).ok())
        .unwrap_or(1.);
    let origin = point(
        px(x / scale) - MENU_SIZE.width,
        px(y / scale) - MENU_SIZE.height,
    );
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin,
                size: MENU_SIZE,
            })),
            titlebar: None,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            focus: true,
            show: true,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| TrayMenu::new(window, cx)),
    );
}

struct TrayMenu {
    _activation: Subscription,
}

impl TrayMenu {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        make_topmost(window);
        let opened = std::time::Instant::now();
        let activation = cx.observe_window_activation(window, move |_, window, _| {
            if !window.is_window_active() && opened.elapsed() > Duration::from_millis(200) {
                window.remove_window();
            }
        });
        Self {
            _activation: activation,
        }
    }

    fn item(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .h(px(28.))
            .px_2()
            .flex()
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().secondary))
            .child(label.into())
            .on_click(move |_, window, cx| {
                on_click(window, cx);
                window.remove_window();
            })
    }
}

impl Render for TrayMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let rule = move || div().h(px(1.)).my_1().bg(border);
        v_flex()
            .size_full()
            .p_1()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius_lg)
            .text_sm()
            .font_family(BODY_FONT)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .h(px(28.))
                    .px_2()
                    .gap_2()
                    .items_center()
                    .child(img("images/letronna.png").size(px(16.)).rounded(px(3.)))
                    .child(
                        div()
                            .font_family(HEADING_FONT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(BRAND),
                    ),
            )
            .child(rule())
            .child(self.item("updates", "Check for updates", cx, |_, cx| {
                cx.open_url(RELEASES)
            }))
            .child(rule())
            .child(self.item("quit", format!("Quit {BRAND}"), cx, |_, cx| cx.quit()))
            .with_animation(
                "tray-menu",
                Animation::new(std::time::Duration::from_millis(160))
                    .with_easing(gpui::ease_out_quint()),
                |el, delta| el.opacity(delta).mt(px(8. * (1. - delta))),
            )
    }
}

pub(crate) fn hide(window: &mut Window) {
    set_visible(window, false);
}

fn hwnd(window: &Window) -> Option<*mut core::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut core::ffi::c_void),
        _ => None,
    }
}

fn make_topmost(window: &Window) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow, SetWindowPos,
    };
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        SetForegroundWindow(hwnd);
    }
}

fn set_visible(window: &Window, visible: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    let Some(hwnd) = hwnd(window) else {
        return;
    };
    unsafe {
        ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
        if visible {
            SetForegroundWindow(hwnd);
        }
    }
}
