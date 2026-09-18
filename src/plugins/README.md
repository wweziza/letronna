# Plugins

A plugin is a self-contained integration that reacts to what happens in the app.
The app never calls a plugin directly. It emits `AppEvent`s onto a bus; the
registry fans each event out to every **enabled** plugin. Adding one is a single
file here plus one line in `main` — nothing else in the app changes.

## The `Plugin` trait

```rust
pub(crate) trait Plugin: 'static {
    fn id(&self) -> &'static str;          // stable config key, never change once shipped
    fn name(&self) -> &'static str;        // shown in Settings › Plugins
    fn description(&self) -> &'static str; // one line, shown under the name
    fn init(&self, cx: &mut App);          // called once at startup: spawn threads, set up
    fn handle(&self, event: &AppEvent);    // react to an event; only called while enabled
    fn set_enabled(&self, enabled: bool);  // turn on/off: tear down or resume your own work
    fn enabled(&self) -> bool;             // current state
    fn builtin(&self) -> bool { true }     // false for externally loaded plugins
}
```

Rules of the trait:

- **`handle` must be cheap and non-blocking.** It runs on the UI thread. Do real
  work on a background thread you spawn in `init`, and send it messages over a
  channel — see `discord.rs` for the pattern.
- **`set_enabled` must actually stop the work.** When disabled, clear whatever
  you put out into the world (a tray item, a presence, a socket) and ignore
  events until re-enabled.
- **`id` is forever.** It is the key in `plugins.json`. Renaming it resets every
  user's on/off choice.

## Events

`AppEvent` is the whole surface a plugin sees today:

```rust
pub(crate) enum AppEvent {
    ChatStarted { model: String, title: String }, // a reply began
    ChatIdle,                                      // the reply finished or failed
    PresenceMode(PresenceMode),                    // user changed the Discord detail level
}
```

To give plugins a new hook, add a variant here and emit it from the app with
`plugins::emit(cx, AppEvent::…)`. Keep variants coarse and app-level (a session
opened, a tool ran, the window focused), not UI details.

## Lifecycle

1. `main` builds the plugin list: `vec![Box::new(MyPlugin::new()), …]`.
2. `plugins::init` reads `plugins.json`, calls `set_enabled` with the saved
   choice (default on), then `init` on each, and stores the registry.
3. During the session the app calls `plugins::emit`; enabled plugins get `handle`.
4. The Plugins settings page calls `plugins::set_enabled`, which flips the plugin
   and rewrites `plugins.json`.

## State and storage

- The on/off map lives in `plugins.json` under the app data dir
  (`%LOCALAPPDATA%\Letronna` on Windows). The app owns this file; a plugin never
  writes it.
- A plugin that needs its own storage should write its own file in the same data
  dir, keyed by its `id`, and read it in `init`.

## Writing one

```rust
// src/plugins/hello.rs
use super::{AppEvent, Plugin};
use crate::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct Hello {
    enabled: AtomicBool,
}

impl Hello {
    pub(crate) fn new() -> Self {
        Self { enabled: AtomicBool::new(true) }
    }
}

impl Plugin for Hello {
    fn id(&self) -> &'static str { "hello" }
    fn name(&self) -> &'static str { "Hello" }
    fn description(&self) -> &'static str { "Logs when a reply starts." }
    fn init(&self, _cx: &mut App) {}
    fn handle(&self, event: &AppEvent) {
        if let AppEvent::ChatStarted { model, .. } = event {
            eprintln!("hello: chatting with {model}");
        }
    }
    fn set_enabled(&self, on: bool) { self.enabled.store(on, Ordering::Relaxed); }
    fn enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }
}
```

Then register it in `src/main.rs`:

```rust
plugins::init(cx, vec![
    Box::new(plugins::discord::DiscordPresence::new(plugins::PresenceMode::Detailed)),
    Box::new(plugins::hello::Hello::new()),
]);
```

Add `pub(crate) mod hello;` to `mod.rs`, and it shows up in Settings › Plugins
with a toggle, a "Built in" badge, and persistence — no other wiring.

## External plugins (planned)

Built-in plugins are compiled into the app. Loading third-party plugins at
runtime is a separate, sandboxed path: WASM modules in a `plugins/` folder next
to the executable, each with a `manifest.json` (id, name, description,
permissions) and a capability-gated host API. Such a plugin reports
`builtin() == false` and shows an "External" badge. This is not built yet; the
trait and the event bus are the seam it will plug into.
