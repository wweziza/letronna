use crate::prelude::*;

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Conversation {
    pub(crate) title: String,
    pub(crate) messages: Vec<Message>,
    #[serde(default)]
    pub(crate) updated: u64,
    #[serde(default)]
    pub(crate) pinned: bool,
}
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Store {
    pub(crate) conversations: Vec<Conversation>,
    pub(crate) active: usize,
    pub(crate) endpoint: String,
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) gateway: String,
}

pub(crate) fn data_path() -> PathBuf {
    backend::data_dir().join("chats.json")
}

pub(crate) fn keys_path() -> PathBuf {
    backend::data_dir().join("keys.json")
}

pub(crate) fn load_keys() -> std::collections::HashMap<String, String> {
    std::fs::read(keys_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub(crate) fn bucket(updated: u64, now: u64) -> &'static str {
    let age = now.saturating_sub(updated);
    match age / 86_400 {
        0 => "TODAY",
        1 => "YESTERDAY",
        2..=6 => "EARLIER THIS WEEK",
        7..=13 => "LAST WEEK",
        _ => "OLDER",
    }
}

pub(crate) fn age_label(updated: u64, now: u64) -> String {
    let age = now.saturating_sub(updated);
    if updated == 0 {
        String::new()
    } else if age < 3600 {
        format!("{}m", (age / 60).max(1))
    } else if age < 86_400 {
        format!("{}h", age / 3600)
    } else {
        format!("{}d", age / 86_400)
    }
}
