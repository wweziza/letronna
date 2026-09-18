use crate::prelude::*;

pub(crate) const BRAND: &str = "Letronna";
pub(crate) const AILE: &str = "https://api.aile.sh/v1";

pub(crate) const GREETINGS: &[&str] = &[
    "Letronna is ready.",
    "Letronna is listening.",
    "What are we building today?",
    "Tell Letronna the goal.",
    "Ready when you are.",
];

actions!(letronna, [NewSession]);

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Page {
    Chat,
    Skills,
    Messaging,
    Artifacts,
    Jobs,
}
impl Page {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Page::Chat => "Chat",
            Page::Skills => "Skills",
            Page::Messaging => "Messaging",
            Page::Artifacts => "Artifacts",
            Page::Jobs => "Scheduled jobs",
        }
    }
    pub(crate) fn blurb(self) -> &'static str {
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

pub(crate) struct Chat {
    pub(crate) store: Store,
    pub(crate) composer: Entity<InputState>,
    pub(crate) search: Entity<InputState>,
    pub(crate) endpoint: Entity<InputState>,
    pub(crate) model: Entity<InputState>,
    pub(crate) key: Entity<InputState>,
    pub(crate) models: Vec<String>,
    pub(crate) model_open: bool,
    pub(crate) model_search: Entity<InputState>,
    pub(crate) keys: std::collections::HashMap<String, String>,
    pub(crate) gateway_pick: Entity<String>,
    pub(crate) settings_page: Entity<&'static str>,
    pub(crate) rename: Entity<InputState>,
    pub(crate) page: Page,
    pub(crate) sidebar_open: bool,
    pub(crate) greeting: &'static str,
    pub(crate) free_remaining: Option<u64>,
    pub(crate) loading_models: bool,
    pub(crate) gateway_error: Option<String>,
    pub(crate) busy: bool,
    pub(crate) partial: String,
    pub(crate) error: Option<String>,
    pub(crate) scroll: ScrollHandle,
    pub(crate) _subscriptions: Vec<Subscription>,
}

impl Chat {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        let settings_page = cx.new(|_| "Gateways");
        let rename = cx.new(|cx| InputState::new(window, cx).placeholder("Session name"));
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
            gateway_error: None,
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
            settings_page,
            rename,
            busy: false,
            partial: String::new(),
            error,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.load_models(window, cx);
        this
    }

    pub(crate) fn load_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_models {
            return;
        }
        self.loading_models = true;
        let base = self.endpoint.read(cx).value().to_string();
        let key = self.key.read(cx).value().to_string();
        let aile = base.trim().trim_end_matches('/') == AILE;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = if aile && key.trim().is_empty() {
                backend::free_models()
            } else if aile {
                let mut models = backend::free_models().unwrap_or_default();
                match backend::list_models(&base, &key) {
                    Ok(paid) => {
                        for m in paid {
                            if !models.contains(&m) {
                                models.push(m);
                            }
                        }
                        Ok(models)
                    }
                    Err(e) if models.is_empty() => Err(e),
                    Err(_) => Ok(models),
                }
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
                        Ok(models) => {
                            this.models = models;
                            this.gateway_error = None;
                        }
                        Err(error) => {
                            this.gateway_error = Some(error.clone());
                            this.error = Some(error);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn choose_model(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn save(&mut self) {
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

    pub(crate) fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                        .w_full()
                        .when(self.sidebar_open, |d| d.max_w(px(760.)))
                        .mx_auto()
                        .px_6()
                        .pb_2()
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().danger.opacity(0.15))
                                .text_color(cx.theme().danger)
                                .text_sm()
                                .child(error),
                        ),
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
