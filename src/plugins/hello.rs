//! A minimal example plugin. It reacts to an event (counts replies) and
//! contributes its own settings control (a toggle) through the `settings` hook.
//! Copy this file as the starting point for a real plugin.
//!
//! Hooks are just trait methods. Rust has no mixins; the app calls the methods
//! the plugin implements. The plugin owns its state in `Arc`s and moves clones
//! into its UI closures, so nothing external is needed to wire it up.

use super::{AppEvent, Plugin};
use crate::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(crate) struct Hello {
    enabled: Arc<AtomicBool>,
    /// The plugin's own setting: whether to log each reply.
    log: Arc<AtomicBool>,
    replies: Arc<AtomicU64>,
}

impl Hello {
    pub(crate) fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            log: Arc::new(AtomicBool::new(true)),
            replies: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Plugin for Hello {
    fn id(&self) -> &'static str {
        "hello"
    }
    fn name(&self) -> &'static str {
        "Hello World"
    }
    fn description(&self) -> &'static str {
        "Example plugin: counts replies and adds its own toggle below."
    }

    fn init(&self, _cx: &mut App) {}

    fn handle(&self, event: &AppEvent) {
        if let AppEvent::ChatStarted { model, .. } = event {
            let n = self.replies.fetch_add(1, Ordering::Relaxed) + 1;
            if self.log.load(Ordering::Relaxed) {
                eprintln!("[hello] reply #{n} with {model}");
            }
        }
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// The UI hook. The app renders this under the plugin's row on the Plugins
    /// page. The closure captures the plugin's own `Arc` state directly.
    fn settings(&self, cx: &App) -> Option<AnyElement> {
        let logging = self.log.load(Ordering::Relaxed);
        let count = self.replies.load(Ordering::Relaxed);
        let log = self.log.clone();
        Some(
            v_flex()
                .gap_2()
                .pt_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Seen {count} replies this session.")),
                )
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_sm().child("Log replies to the console"))
                        .child(
                            div().cursor_pointer().child(
                                gpui_component::switch::Switch::new("hello-log")
                                    .checked(logging)
                                    .on_click(move |_, _, cx| {
                                        let now = !log.load(Ordering::Relaxed);
                                        log.store(now, Ordering::Relaxed);
                                        cx.refresh_windows();
                                    }),
                            ),
                        ),
                )
                .into_any_element(),
        )
    }
}
