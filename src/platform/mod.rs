//! Windows-only OS integration: install/uninstall registration, the tray icon,
//! and low-level window helpers. Everything here is gated to Windows.

pub(crate) mod install;
pub(crate) mod tray;
pub(crate) mod window;
