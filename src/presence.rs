//! Discord rich presence. Runs the blocking IPC on its own thread and receives
//! state updates over a channel, so the UI thread never blocks on Discord.

use crate::prelude::*;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Discord application id. Override with LETRONNA_DISCORD_APP_ID.
/// The large image asset must be uploaded to that app's Art Assets as `letronna`.
const APP_ID: &str = "1420000000000000000";

pub(crate) enum Presence {
    Idle,
    Chatting(String),
}

struct Bridge(Sender<Presence>);
impl Global for Bridge {}

pub(crate) fn init(cx: &mut App) {
    let app_id = std::env::var("LETRONNA_DISCORD_APP_ID").unwrap_or_else(|_| APP_ID.into());
    let (tx, rx) = channel::<Presence>();
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    std::thread::spawn(move || {
        let mut client = DiscordIpcClient::new(&app_id);
        let mut connected = client.connect().is_ok();
        let mut state = Presence::Idle;
        loop {
            match rx.recv_timeout(Duration::from_secs(15)) {
                Ok(next) => state = next,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break,
            }
            if !connected {
                connected = client.connect().is_ok();
            }
            if !connected {
                continue;
            }
            let (state_line, small) = match &state {
                Presence::Idle => ("Idle".to_string(), "Ready"),
                Presence::Chatting(model) => (format!("Chatting · {model}"), "Generating"),
            };
            let activity = activity::Activity::new()
                .details("Letronna")
                .state(&state_line)
                .assets(
                    activity::Assets::new()
                        .large_image("letronna")
                        .large_text("Letronna")
                        .small_text(small),
                )
                .timestamps(activity::Timestamps::new().start(start));
            if client.set_activity(activity).is_err() {
                connected = false;
                let _ = client.close();
            }
        }
        let _ = client.close();
    });

    cx.set_global(Bridge(tx));
}

pub(crate) fn set(cx: &App, presence: Presence) {
    if let Some(bridge) = cx.try_global::<Bridge>() {
        let _ = bridge.0.send(presence);
    }
}
