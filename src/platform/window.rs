//! Win32 window helpers shared by the tray and other platform code.

use crate::prelude::*;

pub(crate) fn hwnd(window: &Window) -> Option<*mut core::ffi::c_void> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut core::ffi::c_void),
        _ => None,
    }
}

/// Bring the window in front of the notification-area flyout.
pub(crate) fn make_topmost(window: &Window) {
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

pub(crate) fn set_visible(window: &Window, visible: bool) {
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

/// Hide the window to the tray instead of closing the app.
pub(crate) fn hide(window: &mut Window) {
    set_visible(window, false);
}
