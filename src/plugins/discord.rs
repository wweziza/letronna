//! Discord rich presence, as a [`Plugin`]. The blocking IPC runs on its own
//! thread; `handle` only forwards a message down a channel, so the UI never
//! waits on Discord.

use super::{AppEvent, Plugin};
use crate::prelude::*;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Discord application id. Override with LETRONNA_DISCORD_APP_ID.
/// The large image asset must be uploaded to that app's Art Assets as `letronna`.
const APP_ID: &str = "1550477901332484147";
const SITE: &str = "https://letronna.gadl.us";

/// Shown as the second line when activity is hidden. One is picked at random.
const PHRASES: &[&str] = &[
    "Cooking something…",
    "Tinkering away…",
    "Deep in thought…",
    "Making things…",
    "In the zone…",
    "Poking at ideas…",
    "Heads down…",
    "Somewhere in a chat…",
    "Doing something…",
    "Chasing a thought…",
    "Building quietly…",
    "Lost in a prompt…",
];

/// How much of the activity to share on Discord.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresenceMode {
    /// Show the conversation and model.
    Detailed,
    /// Show only that Letronna is open, with a vague status.
    Minimal,
    /// No presence at all.
    Off,
}

impl PresenceMode {
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "minimal" => Self::Minimal,
            "off" => Self::Off,
            _ => Self::Detailed,
        }
    }
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Detailed => "detailed",
            Self::Minimal => "minimal",
            Self::Off => "off",
        }
    }
}

/// Messages sent to the presence thread.
enum Update {
    Idle,
    Active { model: String, title: String },
    Mode(PresenceMode),
}

/// The plugin holds the channel to its worker thread.
pub(crate) struct DiscordPresence {
    tx: Sender<Update>,
    initial: PresenceMode,
}

impl DiscordPresence {
    pub(crate) fn new(initial: PresenceMode) -> Self {
        let (tx, rx) = channel();
        spawn_worker(rx, initial);
        Self { tx, initial }
    }
}

impl Plugin for DiscordPresence {
    fn init(&self, _cx: &mut App) {
        let _ = self.tx.send(Update::Mode(self.initial));
    }

    fn handle(&self, event: &AppEvent) {
        let update = match event {
            AppEvent::ChatStarted { model, title } => Update::Active {
                model: model.clone(),
                title: title.clone(),
            },
            AppEvent::ChatIdle => Update::Idle,
            AppEvent::PresenceMode(mode) => Update::Mode(*mode),
        };
        let _ = self.tx.send(update);
    }
}

fn pick_phrase() -> &'static str {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    PHRASES[n % PHRASES.len()]
}

fn spawn_worker(rx: std::sync::mpsc::Receiver<Update>, mode: PresenceMode) {
    let app_id = std::env::var("LETRONNA_DISCORD_APP_ID").unwrap_or_else(|_| APP_ID.into());
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    std::thread::spawn(move || {
        let mut client = DiscordIpcClient::new(&app_id);
        let mut connected = client.connect().is_ok();
        let mut busy = false;
        let mut model = String::new();
        let mut title = String::new();
        let mut phrase = pick_phrase();
        let mut mode = mode;
        loop {
            match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(Update::Mode(m)) => {
                    mode = m;
                    phrase = pick_phrase();
                }
                Ok(Update::Active { model: m, title: t }) => {
                    busy = true;
                    model = m;
                    title = t;
                }
                Ok(Update::Idle) => {
                    busy = false;
                    phrase = pick_phrase();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break,
            }
            if mode == PresenceMode::Off {
                if connected {
                    let _ = client.clear_activity();
                }
                continue;
            }
            if !connected {
                connected = client.connect().is_ok();
                if !connected {
                    continue;
                }
            }
            let (details, state_line, small_image, small_text) = if mode == PresenceMode::Minimal {
                (
                    "In Letronna".to_string(),
                    phrase.to_string(),
                    if busy { "generating" } else { "idle" }.to_string(),
                    if busy { "Working" } else { "Open" }.to_string(),
                )
            } else {
                let convo = if title.is_empty() {
                    "New session".to_string()
                } else {
                    title.clone()
                };
                let model_line = if model.is_empty() {
                    "No model selected".to_string()
                } else {
                    model.clone()
                };
                let (badge, badge_text) = if busy {
                    ("generating", "Generating")
                } else {
                    ("idle", "Idle")
                };
                (convo, model_line, badge.to_string(), badge_text.to_string())
            };
            let mut activity = activity::Activity::new()
                .activity_type(activity::ActivityType::Competing)
                .details(&details)
                .state(&state_line)
                .assets(
                    activity::Assets::new()
                        .large_image("letronna")
                        .large_text("Letronna")
                        .small_image(&small_image)
                        .small_text(&small_text),
                )
                .buttons(vec![activity::Button::new("Get Letronna", SITE)]);
            if busy {
                activity = activity.timestamps(activity::Timestamps::new().start(start));
            }
            if client.set_activity(activity).is_err() {
                connected = false;
                let _ = client.close();
            }
        }
        let _ = client.close();
    });
}
