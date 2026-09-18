//! Optional, self-contained integrations behind a common [`Plugin`] trait.
//!
//! The app never talks to a plugin directly. It emits an [`AppEvent`]; the
//! registry fans it out to every enabled plugin. A plugin declares its identity
//! and reacts to events; it can be toggled on or off, and that choice is saved.
//!
//! To write one: make a struct in `src/plugins/`, implement [`Plugin`], and add
//! it to the `vec![...]` in `main`. Nothing else in the app needs to change.

pub(crate) mod discord;
pub(crate) mod hello;

use crate::prelude::*;
pub(crate) use discord::PresenceMode;

/// Stable id of the Discord presence plugin.
pub(crate) const DISCORD: &str = "discord-presence";
use std::collections::HashMap;

/// Things the app announces that a plugin may care about. Extend this enum to
/// give plugins new hooks (new sessions, tool calls, window focus, …).
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
/// cheap; do real work on a background thread set up in `init`. Enable/disable
/// uses interior mutability so the registry can stay immutable.
pub(crate) trait Plugin: 'static {
    /// Stable identifier, used as the config key. Never change it once shipped.
    fn id(&self) -> &'static str;
    /// Human name shown in Settings.
    fn name(&self) -> &'static str;
    /// One-line description shown in Settings.
    fn description(&self) -> &'static str;
    /// Spawn threads and set up. Called once at startup.
    fn init(&self, cx: &mut App);
    /// React to an app event. Only called while enabled.
    fn handle(&self, event: &AppEvent);
    /// Turn the plugin on or off. It must tear down or resume its own work.
    fn set_enabled(&self, enabled: bool);
    /// Current on/off state.
    fn enabled(&self) -> bool;
    /// True for plugins compiled into the app; false for ones loaded externally.
    fn builtin(&self) -> bool {
        true
    }
    /// Optional settings UI, shown under the plugin on the Plugins page. This is
    /// the hook a plugin uses to add its own controls. Read-only `cx` is enough
    /// to build elements; interactive closures act later with `&mut App`.
    fn settings(&self, _cx: &App) -> Option<AnyElement> {
        None
    }
}

/// A snapshot of a plugin for the Settings list.
pub(crate) struct PluginInfo {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) enabled: bool,
    pub(crate) builtin: bool,
}

struct Registry(Vec<Box<dyn Plugin>>);
impl Global for Registry {}

fn config_path() -> PathBuf {
    backend::data_dir().join("plugins.json")
}

fn load_enabled() -> HashMap<String, bool> {
    std::fs::read(config_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_enabled(map: &HashMap<String, bool>) {
    let _ = std::fs::create_dir_all(backend::data_dir())
        .and_then(|_| std::fs::write(config_path(), serde_json::to_vec_pretty(map).unwrap()));
}

/// Install every plugin, applying the saved enabled state, and keep them for the
/// life of the app.
pub(crate) fn init(cx: &mut App, plugins: Vec<Box<dyn Plugin>>) {
    let saved = load_enabled();
    for plugin in &plugins {
        plugin.set_enabled(*saved.get(plugin.id()).unwrap_or(&true));
        plugin.init(cx);
    }
    cx.set_global(Registry(plugins));
}

/// Fan an event out to every enabled plugin. Safe from any `&App` context.
pub(crate) fn emit(cx: &App, event: AppEvent) {
    if let Some(registry) = cx.try_global::<Registry>() {
        for plugin in &registry.0 {
            if plugin.enabled() {
                plugin.handle(&event);
            }
        }
    }
}

/// Snapshot of all installed plugins for the Settings list.
pub(crate) fn list(cx: &App) -> Vec<PluginInfo> {
    cx.try_global::<Registry>()
        .map(|r| {
            r.0.iter()
                .map(|p| PluginInfo {
                    id: p.id(),
                    name: p.name(),
                    description: p.description(),
                    enabled: p.enabled(),
                    builtin: p.builtin(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Render one plugin's own settings UI, if it has any.
pub(crate) fn render_settings(cx: &App, id: &str) -> Option<AnyElement> {
    let registry = cx.try_global::<Registry>()?;
    registry
        .0
        .iter()
        .find(|p| p.id() == id)
        .and_then(|p| p.settings(cx))
}

/// Toggle a plugin and persist the choice.
pub(crate) fn set_enabled(cx: &App, id: &str, enabled: bool) {
    if let Some(registry) = cx.try_global::<Registry>() {
        let mut map = load_enabled();
        for plugin in &registry.0 {
            if plugin.id() == id {
                plugin.set_enabled(enabled);
                map.insert(id.to_string(), enabled);
            } else {
                map.insert(plugin.id().to_string(), plugin.enabled());
            }
        }
        save_enabled(&map);
    }
}
