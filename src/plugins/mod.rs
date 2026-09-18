//! Optional, self-contained integrations behind a common [`Plugin`] trait.
//!
//! The app never talks to a plugin directly. It emits an [`AppEvent`]; the
//! registry fans it out to every registered plugin. Adding an integration is
//! one file plus one line in `main`, and removing one touches nothing else.

pub(crate) mod discord;

use crate::prelude::*;
pub(crate) use discord::PresenceMode;

/// Things the app announces that a plugin may care about.
#[derive(Clone)]
pub(crate) enum AppEvent {
    /// A reply is being generated in the given session.
    ChatStarted { model: String, title: String },
    /// The current reply finished (or failed).
    ChatIdle,
    /// The user changed the Discord presence mode.
    PresenceMode(PresenceMode),
}

/// A plugin reacts to events without blocking the UI thread. `handle` must be
/// cheap; do real work on a background thread set up in `init`.
pub(crate) trait Plugin: 'static {
    fn init(&self, cx: &mut App);
    fn handle(&self, event: &AppEvent);
}

struct Registry(Vec<Box<dyn Plugin>>);
impl Global for Registry {}

/// Install every plugin and keep them for the life of the app.
pub(crate) fn init(cx: &mut App, plugins: Vec<Box<dyn Plugin>>) {
    for plugin in &plugins {
        plugin.init(cx);
    }
    cx.set_global(Registry(plugins));
}

/// Fan an event out to every plugin. Safe from any `&App` context.
pub(crate) fn emit(cx: &App, event: AppEvent) {
    if let Some(registry) = cx.try_global::<Registry>() {
        for plugin in &registry.0 {
            plugin.handle(&event);
        }
    }
}
