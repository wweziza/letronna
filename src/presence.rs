//! Discord rich presence. Runs the blocking IPC on its own thread and receives
//! state updates over a channel, so the UI thread never blocks on Discord.

use crate::prelude::*;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Discord application id. Override with LETRONNA_DISCORD_APP_ID.
/// The large image asset must be uploaded to that app's Art Assets as `letronna`.
const APP_ID: &str = "1550477901332484147";
const REPO: &str = "https://github.com/wweziza/letronna";

pub(crate) enum Update {
    Idle,
    Chatting(String),
    Mode(PresenceMode),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresenceMode {
    /// Show what Letronna is doing, including the model.
    Detailed,
    /// Show that Letronna is open, but not the activity.
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
        let mut mode = mode;
        loop {
            match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(Update::Mode(m)) => mode = m,
                Ok(next) => state = next,
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
            let detail = mode == PresenceMode::Detailed;
            let busy = matches!(state, Update::Chatting(_));
            // Line 1 (details): what it is doing. Line 2 (state): the specifics.
            let (details, state_line, small_image, small_text) = match (&state, detail) {
                (Update::Chatting(model), true) => {
                    ("Chatting", model.clone(), "generating", "Generating")
                }
                (Update::Chatting(_), false) => (
                    "Busy",
                    "Working on a reply".into(),
                    "generating",
                    "Generating",
                ),
                (_, true) => ("Idle", "Ready to help".into(), "idle", "Ready"),
                (_, false) => ("Open", "Native GPUI agent".into(), "idle", "Ready"),
            };
            let _ = busy;
            let activity = activity::Activity::new()
                .activity_type(activity::ActivityType::Playing)
                .details(details)
                .state(&state_line)
                .assets(
                    activity::Assets::new()
                        .large_image("letronna")
                        .large_text("Letronna · native GPUI agent")
                        .small_image(small_image)
                        .small_text(small_text),
                )
                .timestamps(activity::Timestamps::new().start(start))
                .buttons(vec![activity::Button::new("Get Letronna", REPO)]);
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

pub(crate) fn set_chatting(cx: &App, model: String) {
    send(cx, Update::Chatting(model));
}

pub(crate) fn set_mode(cx: &App, mode: PresenceMode) {
    send(cx, Update::Mode(mode));
}
