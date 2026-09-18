#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod backend;
use backend::{Connection, Event, Message};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, IconNamed, Root, Sizable, Theme, ThemeMode, TitleBar,
    WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    text::TextView,
    v_flex,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

const BRAND: &str = "Letronna";
const AILE: &str = "https://api.aile.sh/v1";
const HEADING_FONT: &str = "Segoe UI Variable Display";
const BODY_FONT: &str = "Segoe UI Variable Text";
const WORDMARK_FONT: &str = "Georgia";
const GREETINGS: &[&str] = &[
    "Letronna is ready.",
    "Letronna is listening.",
    "What are we building today?",
    "Tell Letronna the goal.",
    "Ready when you are.",
];

struct Gateway {
    name: &'static str,
    base: &'static str,
    needs_key: bool,
}

const GATEWAYS: &[Gateway] = &[
    Gateway {
        name: "AILE Free",
        base: AILE,
        needs_key: false,
    },
    Gateway {
        name: "AILE",
        base: AILE,
        needs_key: true,
    },
    Gateway {
        name: "OpenAI",
        base: "https://api.openai.com/v1",
        needs_key: true,
    },
    Gateway {
        name: "OpenRouter",
        base: "https://openrouter.ai/api/v1",
        needs_key: true,
    },
    Gateway {
        name: "Ollama",
        base: "http://localhost:11434/v1",
        needs_key: false,
    },
    Gateway {
        name: "Custom",
        base: "",
        needs_key: false,
    },
];

fn gateway(name: &str) -> &'static Gateway {
    GATEWAYS
        .iter()
        .find(|g| g.name == name)
        .unwrap_or(&GATEWAYS[0])
}

actions!(letronna, [NewSession]);

struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        macro_rules! svg { ($($n:literal),*) => { match path {
            $(concat!("icons/", $n, ".svg") => Some(Cow::Borrowed(include_bytes!(concat!("../assets/icons/", $n, ".svg")))),)*
            _ => None,
        } } }
        Ok(svg!(
            "plus",
            "close",
            "window-close",
            "window-minimize",
            "window-maximize",
            "window-restore",
            "minus",
            "settings",
            "arrow-up",
            "copy",
            "panel-left-close",
            "panel-left-open",
            "chevron-down",
            "check",
            "loader-circle",
            "bot",
            "mic",
            "chevrons-up-down",
            "inbox",
            "send",
            "sparkles",
            "message-square",
            "file",
            "file-text",
            "clock",
            "search",
            "pin",
            "folder",
            "image",
            "square-pen",
            "list-filter"
        ))
    }
    fn list(&self, _: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Copy)]
enum AppIcon {
    Mic,
    Sparkles,
    MessageSquare,
    File,
    FileText,
    Clock,
    Search,
    Folder,
    Image,
    SquarePen,
}
impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            AppIcon::Mic => "icons/mic.svg",
            AppIcon::Sparkles => "icons/sparkles.svg",
            AppIcon::MessageSquare => "icons/message-square.svg",
            AppIcon::File => "icons/file.svg",
            AppIcon::FileText => "icons/file-text.svg",
            AppIcon::Clock => "icons/clock.svg",
            AppIcon::Search => "icons/search.svg",
            AppIcon::Folder => "icons/folder.svg",
            AppIcon::Image => "icons/image.svg",
            AppIcon::SquarePen => "icons/square-pen.svg",
        }
        .into()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Chat,
    Skills,
    Messaging,
    Artifacts,
    Jobs,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Chat => "Chat",
            Page::Skills => "Skills",
            Page::Messaging => "Messaging",
            Page::Artifacts => "Artifacts",
            Page::Jobs => "Scheduled jobs",
        }
    }
    fn blurb(self) -> &'static str {
        match self {
            Page::Chat => "",
            Page::Skills => {
                "Reusable playbooks Letronna can load into a session. Nothing installed yet."
            }
            Page::Messaging => {
                "Connect Telegram, Discord, Slack or email gateways. No channel configured yet."
            }
            Page::Artifacts => "Files, images and links produced in sessions will collect here.",
            Page::Jobs => "Recurring tasks on a schedule. No jobs yet.",
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Conversation {
    title: String,
    messages: Vec<Message>,
    #[serde(default)]
    updated: u64,
    #[serde(default)]
    pinned: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct Store {
    conversations: Vec<Conversation>,
    active: usize,
    endpoint: String,
    model: String,
    #[serde(default)]
    gateway: String,
}

fn data_path() -> PathBuf {
    backend::data_dir().join("chats.json")
}

fn keys_path() -> PathBuf {
    backend::data_dir().join("keys.json")
}

fn load_keys() -> std::collections::HashMap<String, String> {
    std::fs::read(keys_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn bucket(updated: u64, now: u64) -> &'static str {
    let age = now.saturating_sub(updated);
    match age / 86_400 {
        0 => "TODAY",
        1 => "YESTERDAY",
        2..=6 => "EARLIER THIS WEEK",
        7..=13 => "LAST WEEK",
        _ => "OLDER",
    }
}

fn age_label(updated: u64, now: u64) -> String {
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

struct Chat {
    store: Store,
    composer: Entity<InputState>,
    search: Entity<InputState>,
    endpoint: Entity<InputState>,
    model: Entity<InputState>,
    key: Entity<InputState>,
    models: Vec<String>,
    model_open: bool,
    model_search: Entity<InputState>,
    keys: std::collections::HashMap<String, String>,
    gateway_pick: Entity<String>,
    page: Page,
    sidebar_open: bool,
    greeting: &'static str,
    free_remaining: Option<u64>,
    loading_models: bool,
    busy: bool,
    partial: String,
    error: Option<String>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl Chat {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (mut store, error) = match std::fs::read(data_path()) {
            Ok(bytes) => match serde_json::from_slice::<Store>(&bytes) {
                Ok(store) => (store, None),
                Err(_) => (Store::default(), Some("Saved chats could not be read. The original file is retained until you send or start a chat.".into())),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Store::default(), None),
            Err(_) => (Store::default(), Some("Could not read local chat history.".into())),
        };
        if store.conversations.is_empty() {
            store.conversations.push(Conversation {
                title: "New session".into(),
                updated: backend::now(),
                ..Default::default()
            });
        }
        store.active = store.active.min(store.conversations.len() - 1);
        if store.gateway.is_empty() {
            store.gateway = GATEWAYS[0].name.into();
        }
        if store.endpoint.is_empty() {
            store.endpoint = std::env::var("AGENT_BASE_URL").unwrap_or_else(|_| AILE.into());
        }
        let keys = load_keys();
        let saved_key = keys
            .get(&store.gateway)
            .cloned()
            .or_else(|| std::env::var("AGENT_API_KEY").ok())
            .unwrap_or_default();
        if store.model.is_empty() {
            store.model =
                std::env::var("AGENT_MODEL").unwrap_or_else(|_| "aile-free/gpt-oss-20b".into());
        }
        let greeting = GREETINGS[(backend::now() % GREETINGS.len() as u64) as usize];
        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(1, 6)
                .placeholder(greeting)
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search sessions…"));
        let endpoint =
            cx.new(|cx| InputState::new(window, cx).default_value(store.endpoint.clone()));
        let model = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Model ID")
                .default_value(store.model.clone())
        });
        let key = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("API key")
                .masked(true)
                .default_value(saved_key)
        });
        let model_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search models…"));
        let gateway_pick = cx.new(|_| store.gateway.clone());
        let subscriptions = vec![
            cx.subscribe_in(&model_search, window, |_, _, _: &InputEvent, _, cx| {
                cx.notify()
            }),
            cx.subscribe_in(&composer, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { secondary: false }) {
                    this.send(window, cx);
                }
            }),
            cx.subscribe_in(&search, window, |_, _, _: &InputEvent, _, cx| cx.notify()),
        ];
        let mut this = Self {
            page: Page::Chat,
            sidebar_open: true,
            greeting,
            free_remaining: None,
            loading_models: false,
            store,
            composer,
            search,
            endpoint,
            model,
            key,
            models: Vec::new(),
            model_open: false,
            model_search,
            keys,
            gateway_pick,
            busy: false,
            partial: String::new(),
            error,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.load_models(window, cx);
        this
    }

    fn load_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_models {
            return;
        }
        self.loading_models = true;
        let free = self.store.gateway == GATEWAYS[0].name;
        let base = self.endpoint.read(cx).value().to_string();
        let key = self.key.read(cx).value().to_string();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = if free {
                backend::free_models()
            } else {
                backend::list_models(&base, &key)
            };
            let _ = tx.send_blocking(result);
        });
        let _ = window;
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                let _ = this.update(cx, |this, cx| {
                    this.loading_models = false;
                    match result {
                        Ok(models) => this.models = models,
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn choose_model(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.model
            .update(cx, |input, cx| input.set_value(id.clone(), window, cx));
        if id.starts_with("aile-free/") {
            self.endpoint
                .update(cx, |input, cx| input.set_value(AILE, window, cx));
        }
        self.store.model = id;
        self.model_open = false;
        self.save();
        cx.notify();
    }

    fn model_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.model.read(cx).value().to_string();
        let short = current.rsplit('/').next().unwrap_or(&current).to_string();
        let query = self.model_search.read(cx).value().to_lowercase();
        let mut list = v_flex()
            .id("model-list")
            .max_h(px(320.))
            .overflow_y_scroll()
            .p_1()
            .gap_0p5();
        let mut ids: Vec<&String> = self.models.iter().collect();
        if !self.models.contains(&current) {
            ids.insert(0, &current);
        }
        for (i, id) in ids.into_iter().enumerate() {
            if !query.is_empty() && !id.to_lowercase().contains(&query) {
                continue;
            }
            let pick = id.clone();
            list = list.child(
                Button::new(("model", i))
                    .ghost()
                    .xsmall()
                    .w_full()
                    .justify_start()
                    .label(id.clone())
                    .when(*id == current, |b| b.text_color(cx.theme().primary))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_model(pick.clone(), window, cx)
                    })),
            );
        }
        div()
            .relative()
            .flex_shrink_0()
            .child(
                Button::new("model-trigger")
                    .ghost()
                    .xsmall()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(short)
                            .child(Icon::new(IconName::ChevronDown).size_3()),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.model_open = !this.model_open;
                        if this.model_open {
                            this.model_search.update(cx, |s, cx| {
                                s.set_value("", window, cx);
                                s.focus(window, cx);
                            });
                        }
                        cx.notify();
                    })),
            )
            .when(self.model_open, |this| {
                this.child(
                    div().absolute().top_0().right_0().child(
                        deferred(
                            anchored()
                                .anchor(Corner::BottomRight)
                                .snap_to_window_with_margin(px(8.))
                                .child(
                                    div()
                                        .occlude()
                                        .w(px(340.))
                                        .pb_1p5()
                                        .child(
                                            v_flex()
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded(cx.theme().radius_lg)
                                                .shadow_md()
                                                .child(
                                                    div()
                                                        .p_1()
                                                        .border_b_1()
                                                        .border_color(cx.theme().border)
                                                        .child(
                                                            Input::new(&self.model_search)
                                                                .xsmall()
                                                                .appearance(false)
                                                                .prefix(
                                                                    Icon::new(AppIcon::Search)
                                                                        .size_3()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        ),
                                                                ),
                                                        ),
                                                )
                                                .child(list),
                                        )
                                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                            this.model_open = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .with_priority(1),
                    ),
                )
            })
    }

    fn attach(&mut self, directories: bool, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: !directories,
            directories,
            multiple: true,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    let mut text = String::new();
                    for path in paths {
                        text.push_str(&format!("[attached: {}]\n", path.display()));
                    }
                    this.insert(&text, window, cx);
                });
            }
        })
        .detach();
    }

    fn insert(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |input, cx| {
            let mut value = input.value().to_string();
            value.push_str(text);
            input.set_value(value, window, cx);
        });
    }

    fn save(&mut self) {
        let path = data_path();
        let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let bytes = serde_json::to_vec_pretty(&self.store)?;
            let temp = path.with_extension("tmp");
            std::fs::write(&temp, bytes)?;
            std::fs::rename(temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            self.error =
                Some("Could not save chat history. Your current chat remains in memory.".into());
        }
    }

    fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.store.conversations.push(Conversation {
            title: "New session".into(),
            updated: backend::now(),
            ..Default::default()
        });
        self.store.active = self.store.conversations.len() - 1;
        self.error = None;
        self.page = Page::Chat;
        self.composer
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.save();
        cx.notify();
    }

    fn select_gateway(&mut self, name: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let g = gateway(name);
        self.store.gateway = name.into();
        self.gateway_pick.update(cx, |g, cx| {
            *g = name.into();
            cx.notify();
        });
        if !g.base.is_empty() {
            self.endpoint
                .update(cx, |input, cx| input.set_value(g.base, window, cx));
        }
        let key = self.keys.get(name).cloned().unwrap_or_default();
        self.key
            .update(cx, |input, cx| input.set_value(key, window, cx));
        cx.notify();
    }

    fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store.endpoint = self.endpoint.read(cx).value().to_string();
        self.store.model = self.model.read(cx).value().to_string();
        let key = self.key.read(cx).value().to_string();
        if key.is_empty() {
            self.keys.remove(&self.store.gateway);
        } else {
            self.keys.insert(self.store.gateway.clone(), key);
        }
        if std::fs::create_dir_all(backend::data_dir())
            .and_then(|_| {
                std::fs::write(keys_path(), serde_json::to_vec_pretty(&self.keys).unwrap())
            })
            .is_err()
        {
            self.error = Some("Could not save the API key.".into());
        }
        self.save();
        self.error = None;
        self.load_models(window, cx);
        window.close_dialog(cx);
        cx.notify();
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        let key = self.key.clone();
        let chat = cx.entity();
        let pick = self.gateway_pick.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let label = |text: &'static str, cx: &App| {
                div()
                    .text_xs()
                    .font_family(HEADING_FONT)
                    .text_color(cx.theme().muted_foreground)
                    .child(text)
            };
            let current = pick.read(cx).clone();
            let g = gateway(&current);
            let mut chips = h_flex().flex_wrap().gap_1p5();
            for gw in GATEWAYS {
                let chat = chat.clone();
                let active = gw.name == current;
                chips = chips.child(
                    Button::new(gw.name)
                        .small()
                        .map(|b| if active { b.primary() } else { b.outline() })
                        .label(gw.name)
                        .on_click(move |_, window, cx| {
                            chat.update(cx, |this, cx| this.select_gateway(gw.name, window, cx))
                        }),
                );
            }
            let hint = if g.needs_key {
                "This gateway needs an API key. It is stored locally in keys.json and never in chat history."
            } else if g.name == "Custom" {
                "Any OpenAI-compatible base URL. Add a key if the server requires one."
            } else {
                "No key needed. The key field is optional."
            };
            let save = chat.clone();
            dialog
                .title(div().font_family(HEADING_FONT).child("Settings"))
                .w(window.viewport_size().width * 0.8)
                .child(
                    v_flex()
                        .gap_4()
                        .py_2()
                        .child(v_flex().gap_1p5().child(label("Gateway", cx)).child(chips))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(hint),
                        )
                        .child(v_flex().gap_1().child(label("Base URL", cx)).child(Input::new(&endpoint)))
                        .child(v_flex().gap_1().child(label("API key", cx)).child(Input::new(&key)))
                        .child(v_flex().gap_1().child(label("Default model", cx)).child(Input::new(&model)))
                        .child(
                            h_flex().gap_2().pt_2().child(
                                Button::new("save-settings")
                                    .primary()
                                    .small()
                                    .label("Save and reload models")
                                    .on_click(move |_, window, cx| {
                                        save.update(cx, |this, cx| this.apply_settings(window, cx))
                                    }),
                            ),
                        ),
                )
        });
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let prompt = self.composer.read(cx).value().trim().to_owned();
        if prompt.is_empty() {
            self.composer
                .update(cx, |input, cx| input.set_value("", window, cx));
            return;
        }
        if prompt.chars().count() > 12_000 {
            self.error = Some("Please keep each message under 12,000 characters.".into());
            cx.notify();
            return;
        }
        let connection = Connection {
            endpoint: self.endpoint.read(cx).value().to_string(),
            model: self.model.read(cx).value().to_string(),
            key: self.key.read(cx).value().to_string(),
        };
        if let Err(error) = backend::validate(&connection) {
            self.error = Some(error);
            self.open_settings(window, cx);
            cx.notify();
            return;
        }
        self.store.endpoint = connection.endpoint.clone();
        self.store.model = connection.model.clone();
        let conversation = &mut self.store.conversations[self.store.active];
        if conversation.messages.is_empty() {
            conversation.title = prompt.chars().take(40).collect();
        }
        conversation.updated = backend::now();
        conversation.messages.push(Message {
            role: "user".into(),
            content: prompt,
            model: String::new(),
        });
        let receiver = backend::start(connection, conversation.messages.clone());
        self.composer
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.busy = true;
        self.page = Page::Chat;
        self.partial.clear();
        self.error = None;
        self.scroll.scroll_to_bottom();
        self.save();
        cx.notify();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(event, Event::Finished(_));
                if this
                    .update(cx, |this, cx| {
                        match event {
                            Event::Delta(text) => this.partial.push_str(&text),
                            Event::FreeRemaining(n) => this.free_remaining = Some(n),
                            Event::Finished(result) => {
                                this.busy = false;
                                match result {
                                    Ok(()) => {
                                        let model = this.store.model.clone();
                                        let c = &mut this.store.conversations[this.store.active];
                                        c.updated = backend::now();
                                        c.messages.push(Message {
                                            role: "assistant".into(),
                                            content: std::mem::take(&mut this.partial),
                                            model,
                                        })
                                    }
                                    Err(error) => {
                                        this.partial.clear();
                                        this.error = Some(error);
                                    }
                                }
                                this.save();
                            }
                        }
                        this.scroll.scroll_to_bottom();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if finished {
                    break;
                }
            }
        })
        .detach();
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            h_flex().flex_1().items_center().gap_2().child(
                Button::new("toggle-sidebar")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(if self.sidebar_open {
                        IconName::PanelLeftClose
                    } else {
                        IconName::PanelLeftOpen
                    }))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar_open = !this.sidebar_open;
                        cx.notify();
                    })),
            ),
        )
    }

    fn nav_item(
        &self,
        id: &'static str,
        icon: AppIcon,
        label: &'static str,
        page: Option<Page>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = page.is_some_and(|p| p == self.page);
        h_flex()
            .id(id)
            .h(px(28.))
            .px_2()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .font_family(HEADING_FONT)
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().sidebar_foreground)
            .when(active, |this| this.bg(cx.theme().sidebar_accent))
            .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
            .child(
                Icon::new(icon)
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(div().flex_1().child(label))
            .when_some(page, |this, page| {
                this.on_click(cx.listener(move |this, _, _, cx| {
                    this.page = page;
                    cx.notify();
                }))
            })
    }

    fn section(&self, label: &'static str, cx: &Context<Self>) -> Div {
        div()
            .mt_3()
            .mb_1()
            .px_2()
            .text_xs()
            .font_family(HEADING_FONT)
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().muted_foreground)
            .child(label)
    }

    fn session_row(&self, index: usize, now: u64, cx: &mut Context<Self>) -> impl IntoElement {
        let c = &self.store.conversations[index];
        let active = index == self.store.active && self.page == Page::Chat;
        h_flex()
            .id(("session", index))
            .h(px(26.))
            .px_2()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius)
            .cursor_pointer()
            .text_sm()
            .when(active, |this| this.bg(cx.theme().sidebar_accent))
            .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
            .child(
                div()
                    .size(px(4.))
                    .rounded_full()
                    .flex_shrink_0()
                    .bg(if active {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground.opacity(0.5)
                    }),
            )
            .child(div().flex_1().min_w_0().truncate().child(c.title.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(age_label(c.updated, now)),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if event.modifiers().shift {
                    this.store.conversations[index].pinned ^= true;
                } else if !this.busy {
                    this.store.active = index;
                    this.error = None;
                    this.page = Page::Chat;
                }
                this.save();
                cx.notify();
            }))
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let now = backend::now();
        let query = self.search.read(cx).value().to_lowercase();
        let visible: Vec<usize> = (0..self.store.conversations.len())
            .rev()
            .filter(|&i| {
                query.is_empty()
                    || self.store.conversations[i]
                        .title
                        .to_lowercase()
                        .contains(&query)
            })
            .collect();
        let pinned: Vec<usize> = visible
            .iter()
            .copied()
            .filter(|&i| self.store.conversations[i].pinned)
            .collect();
        let mut sessions = v_flex().gap_0p5();
        let mut last_bucket = "";
        for &i in visible
            .iter()
            .filter(|&&i| !self.store.conversations[i].pinned)
        {
            let b = bucket(self.store.conversations[i].updated, now);
            if b != last_bucket {
                last_bucket = b;
                sessions = sessions.child(
                    div()
                        .mt_2()
                        .mb_0p5()
                        .px_2()
                        .text_xs()
                        .font_family(HEADING_FONT)
                        .text_color(cx.theme().muted_foreground)
                        .child(b),
                );
            }
            sessions = sessions.child(self.session_row(i, now, cx));
        }
        let has_pinned = !pinned.is_empty();
        let mut pinned_list = v_flex().gap_0p5();
        for i in pinned {
            pinned_list = pinned_list.child(self.session_row(i, now, cx));
        }

        v_flex()
            .w(px(250.))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r(px(3.))
            .border_color(cx.theme().sidebar_border)
            .child(
                v_flex()
                    .px_2()
                    .pt_2()
                    .gap_0p5()
                    .child(
                        self.nav_item("new", AppIcon::SquarePen, "New session", None, cx)
                            .child(Kbd::new(Keystroke::parse("ctrl-n").unwrap()).text_xs())
                            .on_click(cx.listener(|this, _, window, cx| this.new_chat(window, cx))),
                    )
                    .child(self.nav_item(
                        "skills",
                        AppIcon::Sparkles,
                        "Skills",
                        Some(Page::Skills),
                        cx,
                    ))
                    .child(self.nav_item(
                        "messaging",
                        AppIcon::MessageSquare,
                        "Messaging",
                        Some(Page::Messaging),
                        cx,
                    ))
                    .child(self.nav_item(
                        "artifacts",
                        AppIcon::File,
                        "Artifacts",
                        Some(Page::Artifacts),
                        cx,
                    ))
                    .child(self.nav_item(
                        "jobs",
                        AppIcon::Clock,
                        "Scheduled jobs",
                        Some(Page::Jobs),
                        cx,
                    )),
            )
            .child(
                div().px_2().pt_3().child(
                    Input::new(&self.search).xsmall().prefix(
                        Icon::new(AppIcon::Search)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    ),
                ),
            )
            .child(
                v_flex()
                    .id("session-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .when(has_pinned, |this| {
                        this.child(self.section("Pinned", cx)).child(pinned_list)
                    })
                    .child(self.section("Sessions", cx))
                    .child(sessions),
            )
            .child(
                div().p_2().child(
                    Button::new("settings")
                        .ghost()
                        .xsmall()
                        .icon(Icon::new(IconName::Settings))
                        .label("Settings")
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                        ),
                ),
            )
    }

    fn message_row(
        &self,
        index: usize,
        message: &Message,
        streaming: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_user = message.role == "user";
        let label: String = if is_user {
            "You".into()
        } else if message.model.is_empty() {
            self.store.model.clone()
        } else {
            message.model.clone()
        };
        let body: AnyElement = if is_user {
            div().child(message.content.clone()).into_any_element()
        } else if message.content.is_empty() {
            div()
                .text_color(cx.theme().muted_foreground)
                .child("Thinking…")
                .into_any_element()
        } else {
            TextView::markdown(("md", index), message.content.clone(), window, cx)
                .selectable(true)
                .into_any_element()
        };
        v_flex()
            .w_full()
            .gap_1p5()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .font_family(HEADING_FONT)
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if is_user {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().primary
                            })
                            .child(label),
                    )
                    .when(streaming, |this| {
                        this.child(div().size(px(6.)).rounded_full().bg(cx.theme().primary))
                    }),
            )
            .child(body)
    }

    fn empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .font_family(WORDMARK_FONT)
                    .italic()
                    .text_size(px(72.))
                    .line_height(px(80.))
                    .child(BRAND),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.greeting),
            )
    }

    fn page_view(&self, page: Page, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .px_8()
            .py_6()
            .gap_2()
            .child(
                div()
                    .text_xl()
                    .font_family(HEADING_FONT)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(page.title()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(page.blurb()),
            )
    }

    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let chat = cx.entity();
        let attach = Button::new("attach")
            .ghost()
            .xsmall()
            .icon(Icon::new(IconName::Plus))
            .disabled(self.busy)
            .dropdown_menu(move |menu, _, _| {
                let files = chat.clone();
                let folder = chat.clone();
                let images = chat.clone();
                let snippet = chat.clone();
                menu.min_w(px(180.))
                    .item(
                        PopupMenuItem::new("Files")
                            .icon(Icon::new(AppIcon::File))
                            .on_click(move |_, window, cx| {
                                files.update(cx, |this, cx| this.attach(false, window, cx))
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Folder")
                            .icon(Icon::new(AppIcon::Folder))
                            .on_click(move |_, window, cx| {
                                folder.update(cx, |this, cx| this.attach(true, window, cx))
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Images")
                            .icon(Icon::new(AppIcon::Image))
                            .on_click(move |_, window, cx| {
                                images.update(cx, |this, cx| this.attach(false, window, cx))
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Prompt snippet")
                            .icon(Icon::new(AppIcon::FileText))
                            .on_click(move |_, window, cx| {
                                snippet.update(cx, |this, cx| {
                                    this.insert(
                                        "Goal:\nContext:\nConstraints:\nDeliverable:\n",
                                        window,
                                        cx,
                                    )
                                })
                            }),
                    )
            });
        h_flex()
            .w_full()
            .items_center()
            .gap_1()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary.opacity(0.4))
            .px_2()
            .py_1()
            .child(attach)
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.composer)
                        .w_full()
                        .appearance(false)
                        .disabled(self.busy),
                ),
            )
            .child(self.model_picker(cx))
            .child(
                Button::new("mic")
                    .ghost()
                    .xsmall()
                    .icon(Icon::new(AppIcon::Mic))
                    .tooltip("Voice input, coming soon"),
            )
            .child(
                Button::new("send")
                    .primary()
                    .xsmall()
                    .rounded_full()
                    .icon(Icon::new(IconName::ArrowUp))
                    .loading(self.busy)
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
            )
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h(px(20.))
            .px_3()
            .gap_2()
            .items_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when_some(self.free_remaining, |this, n| {
                this.child(format!("{n} free replies left"))
            })
            .child(div().flex_1())
            .child(concat!("v", env!("CARGO_PKG_VERSION")))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground.opacity(0.6))
                    .child(env!("GIT_HASH")),
            )
    }
}

impl Render for Chat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.store.active;
        let conversation = &self.store.conversations[active];

        let body: AnyElement = if self.page != Page::Chat {
            self.page_view(self.page, cx).into_any_element()
        } else if conversation.messages.is_empty() && !self.busy {
            self.empty_state(cx).into_any_element()
        } else {
            let mut rows: Vec<AnyElement> = Vec::new();
            for (i, m) in conversation.messages.iter().enumerate() {
                rows.push(self.message_row(i, m, false, window, cx).into_any_element());
            }
            if self.busy {
                let live = Message {
                    role: "assistant".into(),
                    content: self.partial.clone(),
                    model: self.store.model.clone(),
                };
                rows.push(
                    self.message_row(usize::MAX, &live, true, window, cx)
                        .into_any_element(),
                );
            }
            div()
                .id("messages")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(
                    v_flex()
                        .w_full()
                        .when(self.sidebar_open, |d| d.max_w(px(760.)))
                        .mx_auto()
                        .px_6()
                        .py_6()
                        .gap_6()
                        .children(rows),
                )
                .vertical_scrollbar(&self.scroll)
                .into_any_element()
        };

        let main = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(body)
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .mx_6()
                        .mb_2()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().danger.opacity(0.15))
                        .text_color(cx.theme().danger)
                        .text_sm()
                        .child(error),
                )
            })
            .when(self.page == Page::Chat, |this| {
                this.child(
                    div()
                        .w_full()
                        .when(self.sidebar_open, |d| d.max_w(px(760.)))
                        .mx_auto()
                        .px_6()
                        .pb_2()
                        .child(self.composer(cx)),
                )
            })
            .child(self.status_bar(cx));

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(BODY_FONT)
            .text_sm()
            .relative()
            .on_action(cx.listener(|this, _: &NewSession, window, cx| this.new_chat(window, cx)))
            .child(self.title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .when(self.sidebar_open, |this| this.child(self.sidebar(cx)))
                    .child(main),
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn main() {
    Application::new().with_assets(Assets).run(|cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-n", NewSession, None),
            KeyBinding::new(
                "shift-enter",
                gpui_component::input::Enter { secondary: true },
                Some("Input"),
            ),
        ]);
        {
            let t = Theme::global_mut(cx);
            let blue: Hsla = rgb(0x3d6bff).into();
            let bg: Hsla = rgb(0x0c0f15).into();
            let side: Hsla = rgb(0x090c11).into();
            t.primary = blue;
            t.primary_hover = rgb(0x5580ff).into();
            t.primary_active = rgb(0x2f5ae6).into();
            t.ring = blue;
            t.link = blue;
            t.caret = blue;
            t.selection = blue.opacity(0.35);
            t.background = bg;
            t.foreground = rgb(0xd4d8e0).into();
            t.sidebar_foreground = rgb(0xc6cbd5).into();
            t.muted_foreground = rgb(0x7c8493).into();
            t.sidebar = side;
            t.title_bar = side;
            t.title_bar_border = side;
            t.sidebar_accent = rgb(0x151a23).into();
            t.sidebar_border = rgb(0x11151c).into();
            t.secondary = rgb(0x171c25).into();
            t.secondary_hover = rgb(0x1e2430).into();
            t.border = rgb(0x1c2230).into();
            t.input = rgb(0x1c2230).into();
            t.radius = px(4.);
            t.radius_lg = px(6.);
            t.font_family = BODY_FONT.into();
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1100.), px(760.)),
                    cx,
                ))),
                window_min_size: Some(size(px(720.), px(520.))),
                titlebar: Some(TitlebarOptions {
                    title: Some(BRAND.into()),
                    ..TitleBar::title_bar_options()
                }),
                ..Default::default()
            },
            |window, cx| {
                let chat = cx.new(|cx| Chat::new(window, cx));
                cx.new(|cx| Root::new(chat, window, cx))
            },
        )
        .expect("Could not open native application window");
        cx.activate(true);
    });
}
