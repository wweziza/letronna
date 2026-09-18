//! Discord rich presence. Runs the blocking IPC on its own thread and receives
//! state updates over a channel, so the UI thread never blocks on Discord.

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

pub(crate) enum Update {
    Idle,
    Active { model: String, title: String },
    Mode(PresenceMode),
}

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

struct Bridge(Sender<Update>);
impl Global for Bridge {}

fn pick_phrase() -> &'static str {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    PHRASES[n % PHRASES.len()]
}

pub(crate) fn init(cx: &mut App, mode: PresenceMode) {
    let app_id = std::env::var("LETRONNA_DISCORD_APP_ID").unwrap_or_else(|_| APP_ID.into());
    let (tx, rx) = channel::<Update>();
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    std::thread::spawn(move || {
        let mut client = DiscordIpcClient::new(&app_id);
        let mut connected = client.connect().is_ok();
        let mut state = Update::Idle;
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
                    model = m;
                    title = t;
                    state = Update::Active {
                        model: model.clone(),
                        title: title.clone(),
                    };
                }
                Ok(Update::Idle) => {
                    state = Update::Idle;
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
            let busy = matches!(state, Update::Active { .. });
            // Detailed: line 1 is the conversation, line 2 is the model.
            // Minimal: line 1 is "In Letronna", line 2 is a vague status.
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
                if busy {
                    (
                        convo,
                        model_line,
                        "generating".into(),
                        "Generating".to_string(),
                    )
                } else {
                    (convo, model_line, "idle".into(), "Idle".to_string())
                }
            };
            let mut activity = activity::Activity::new()
                .activity_type(activity::ActivityType::Competing)
                .details(&details)
                .state(&state_line)
                .assets(
                    activity::Assets::new()
                        .large_image("letronna")
                        .large_text("Letronna")
                        .small_image(small_image)
                        .small_text(small_text),
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

    cx.set_global(Bridge(tx));
}

fn send(cx: &App, update: Update) {
    if let Some(bridge) = cx.try_global::<Bridge>() {
        let _ = bridge.0.send(update);
    }
}

pub(crate) fn set_idle(cx: &App) {
    send(cx, Update::Idle);
}

pub(crate) fn set_active(cx: &App, model: String, title: String) {
    send(cx, Update::Active { model, title });
}

pub(crate) fn set_mode(cx: &App, mode: PresenceMode) {
    send(cx, Update::Mode(mode));
}
