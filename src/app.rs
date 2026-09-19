use crate::prelude::*;

pub(crate) const BRAND: &str = "Letronna";

pub(crate) const GREETINGS: &[&str] = &[
    "Letronna is ready.",
    "Letronna is listening.",
    "What are we building today?",
    "Tell Letronna the goal.",
    "Ready when you are.",
];

actions!(letronna, [NewSession]);

/// Tool rounds per user message. Enough for a real investigation, small
/// enough that a looping model stops on its own.
const MAX_ROUNDS: usize = 25;

/// Tool calls waiting to run for the current turn. The front one is either
/// running or waiting on the approval card.
pub(crate) struct Pending {
    pub(crate) calls: std::collections::VecDeque<backend::ToolCall>,
    pub(crate) done: Vec<Message>,
    pub(crate) connection: Connection,
}

pub(crate) enum Verdict {
    Allow {
        remember: bool,
    },
    Deny,
    /// The reply to an `ask_user` question.
    Answer(String),
}

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
    pub(crate) presence_pick: Entity<String>,
    pub(crate) settings_page: Entity<&'static str>,
    pub(crate) settings_open: bool,
    pub(crate) settings_backdrop: Option<std::sync::Arc<RenderImage>>,
    pub(crate) settings_closing: bool,
    pub(crate) rename: Entity<InputState>,
    pub(crate) page: Page,
    pub(crate) sidebar_open: bool,
    pub(crate) greeting: &'static str,
    pub(crate) free_remaining: Option<u64>,
    pub(crate) loading_models: bool,
    pub(crate) gateway_error: Option<String>,
    pub(crate) busy: bool,
    pub(crate) partial: String,
    /// The session id currently receiving a streamed reply, if any.
    pub(crate) streaming_id: Option<u64>,
    pub(crate) partial_reasoning: String,
    /// Tool calls the current turn asked for, applied when it finishes.
    pub(crate) pending_calls: Vec<backend::ToolCall>,
    /// Tool rounds in the current send, capped by `MAX_ROUNDS`.
    pub(crate) rounds: usize,
    pub(crate) pending: Option<Pending>,
    /// Tools the user allowed for the rest of this run of the app.
    pub(crate) session_allow: std::collections::HashSet<String>,
    /// Text field on the `ask_user` card.
    pub(crate) answer: Entity<InputState>,
    pub(crate) live_tokens: u64,
    pub(crate) live_per_second: f32,
    pub(crate) thinking_open: std::collections::HashSet<usize>,
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
        store.assign_ids();
        store.active = store.active.min(store.conversations.len() - 1);
        if store.gateway.is_empty() {
            store.gateway = GATEWAYS[0].name.into();
        }
        if store.endpoint.is_empty() {
            store.endpoint = std::env::var("AGENT_BASE_URL").unwrap_or_else(|_| AILE.into());
        }
        let keys = load_keys();
        let saved_key = keys
            .get(key_slot(&store.gateway))
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
        let presence_pick = cx.new(|_| store.presence.clone());
        let settings_page = cx.new(|_| "Gateways");
        let rename = cx.new(|cx| InputState::new(window, cx).placeholder("Session name"));
        let answer = cx.new(|cx| InputState::new(window, cx).placeholder("Your answer…"));
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
            cx.subscribe_in(&answer, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.submit_answer(window, cx);
                }
            }),
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
            presence_pick,
            settings_page,
            settings_open: false,
            settings_backdrop: None,
            settings_closing: false,
            rename,
            busy: false,
            partial: String::new(),
            streaming_id: None,
            partial_reasoning: String::new(),
            pending_calls: Vec::new(),
            rounds: 0,
            pending: None,
            session_allow: std::collections::HashSet::new(),
            answer,
            live_tokens: 0,
            live_per_second: 0.,
            thinking_open: std::collections::HashSet::new(),
            error,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.load_models(window, cx);
        if this.store.presence == "off" {
            plugins::set_enabled(cx, plugins::DISCORD, false);
            this.store.presence = "detailed".into();
            this.save();
        }
        plugins::emit(
            cx,
            plugins::AppEvent::PresenceMode(plugins::PresenceMode::from_str(&this.store.presence)),
        );
        this
    }

    pub(crate) fn load_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading_models {
            return;
        }
        self.loading_models = true;
        let base = self.endpoint.read(cx).value().to_string();
        let key = self.key.read(cx).value().to_string();
        let guest = self.store.gateway == GATEWAYS[0].name;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = if guest {
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
        // A new session usually continues on the same project.
        let workspace = self.store.conversations[self.store.active]
            .workspace
            .clone();
        self.store.conversations.push(Conversation {
            id: self.store.next_id(),
            title: "New session".into(),
            updated: backend::now(),
            workspace,
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
            guest: self.store.gateway == GATEWAYS[0].name,
            tools: self.store.gateway != GATEWAYS[0].name,
            workspace: None,
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
            ..Default::default()
        });
        let target_id = conversation.id;
        self.composer
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.busy = true;
        self.streaming_id = Some(target_id);
        self.rounds = 0;
        self.page = Page::Chat;
        let convo_title = self.store.conversations[self.store.active].title.clone();
        plugins::emit(
            cx,
            plugins::AppEvent::ChatStarted {
                model: self.store.model.clone(),
                title: convo_title,
            },
        );
        self.error = None;
        self.save();
        self.scroll.scroll_to_bottom();
        self.run_turn(connection, cx);
    }

    /// One model call. If it ends in tool calls, run them and go again.
    fn run_turn(&mut self, mut connection: Connection, cx: &mut Context<Self>) {
        let Some(ix) = self.streaming_id.and_then(|id| self.store.index_of(id)) else {
            self.finish(cx);
            return;
        };
        connection.workspace = self.store.conversations[ix].workspace.clone();
        let receiver = backend::start(
            connection.clone(),
            self.store.conversations[ix].messages.clone(),
        );
        self.partial.clear();
        self.partial_reasoning.clear();
        self.pending_calls.clear();
        self.live_tokens = 0;
        self.live_per_second = 0.;
        self.follow();
        cx.notify();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                let finished = matches!(event, Event::Finished(_));
                if this
                    .update(cx, |this, cx| {
                        match event {
                            Event::Delta(text) => this.partial.push_str(&text),
                            Event::Reasoning(text) => this.partial_reasoning.push_str(&text),
                            Event::Usage {
                                completion,
                                per_second,
                            } => {
                                this.live_tokens = completion;
                                this.live_per_second = per_second;
                            }
                            Event::FreeRemaining(n) => this.free_remaining = Some(n),
                            Event::ToolCalls(calls) => this.pending_calls = calls,
                            Event::Finished(Err(error)) => {
                                this.partial.clear();
                                this.partial_reasoning.clear();
                                this.error = Some(error);
                                this.finish(cx);
                            }
                            Event::Finished(Ok(())) => {
                                let calls = std::mem::take(&mut this.pending_calls);
                                let reply = Message {
                                    role: "assistant".into(),
                                    content: std::mem::take(&mut this.partial),
                                    model: this.store.model.clone(),
                                    reasoning: std::mem::take(&mut this.partial_reasoning),
                                    tokens: this.live_tokens,
                                    per_second: this.live_per_second,
                                    tool_calls: calls.clone(),
                                    ..Default::default()
                                };
                                // The session may have been switched or
                                // deleted while the reply streamed.
                                let Some(ix) =
                                    this.streaming_id.and_then(|id| this.store.index_of(id))
                                else {
                                    this.finish(cx);
                                    return;
                                };
                                let c = &mut this.store.conversations[ix];
                                c.updated = backend::now();
                                c.messages.push(reply);
                                this.save();
                                if calls.is_empty() || this.rounds >= MAX_ROUNDS {
                                    this.finish(cx);
                                } else {
                                    this.rounds += 1;
                                    this.run_tools(calls, connection.clone(), cx);
                                }
                            }
                        }
                        this.follow();
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

    /// Queue the requested tools. Each runs in turn; ones that need approval
    /// wait for the card in the chat.
    fn run_tools(
        &mut self,
        calls: Vec<backend::ToolCall>,
        connection: Connection,
        cx: &mut Context<Self>,
    ) {
        self.pending = Some(Pending {
            calls: calls.into(),
            done: Vec::new(),
            connection,
        });
        self.advance_tools(cx);
    }

    /// Run the next queued call, or stop on one that needs the user.
    pub(crate) fn advance_tools(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        let Some(call) = pending.calls.front().cloned() else {
            let Pending {
                done, connection, ..
            } = self.pending.take().unwrap();
            if let Some(ix) = self.streaming_id.and_then(|id| self.store.index_of(id)) {
                self.store.conversations[ix].messages.extend(done);
                self.save();
                self.run_turn(connection, cx);
            } else {
                self.finish(cx);
            }
            return;
        };
        let gated = crate::core::tools::needs_approval(&call.name)
            && (call.name == crate::core::tools::ASK_USER
                || !(self.store.skip_approvals || self.session_allow.contains(&call.name)));
        if gated {
            cx.notify();
            return;
        }
        self.execute_call(call, cx);
    }

    /// Run one call off the UI thread and continue the queue when it returns.
    fn execute_call(&mut self, call: backend::ToolCall, cx: &mut Context<Self>) {
        let ctx = crate::core::tools::ToolContext {
            workspace: self
                .pending
                .as_ref()
                .and_then(|p| p.connection.workspace.clone()),
        };
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(crate::core::tools::run(&call.name, &call.arguments, &ctx));
        });
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                let _ = this.update(cx, |this, cx| this.record_result(result, cx));
            }
        })
        .detach();
    }

    /// Pop the current call, store its result, move on.
    pub(crate) fn record_result(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.as_mut() else {
            return;
        };
        let Some(call) = pending.calls.pop_front() else {
            return;
        };
        let (content, is_error) = match result {
            Ok(out) => (out, false),
            Err(e) => (e, true),
        };
        pending.done.push(Message {
            role: "tool".into(),
            content,
            tool_call_id: call.id,
            name: call.name,
            args: call.arguments,
            is_error,
            ..Default::default()
        });
        self.follow();
        self.advance_tools(cx);
    }

    /// The user answered the approval card.
    pub(crate) fn resolve_call(&mut self, verdict: Verdict, cx: &mut Context<Self>) {
        let Some(call) = self.pending.as_ref().and_then(|p| p.calls.front().cloned()) else {
            return;
        };
        match verdict {
            Verdict::Answer(text) => self.record_result(Ok(text), cx),
            Verdict::Deny => self.record_result(Err(crate::core::tools::DECLINED.into()), cx),
            Verdict::Allow { remember } => {
                if remember {
                    self.session_allow.insert(call.name.clone());
                }
                self.execute_call(call, cx);
            }
        }
    }

    /// Follow the stream only while the user is already at the bottom, so
    /// scrolling up to reread does not snap back on every token.
    pub(crate) fn follow(&self) {
        let slack = px(48.);
        let gap = self.scroll.max_offset().height + self.scroll.offset().y;
        if gap <= slack {
            self.scroll.scroll_to_bottom();
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        self.busy = false;
        self.streaming_id = None;
        self.pending_calls.clear();
        self.pending = None;
        plugins::emit(cx, plugins::AppEvent::ChatIdle);
        cx.notify();
    }
}

impl Render for Chat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(error) = self.error.take() {
            let text = error.clone();
            window.push_notification(
                gpui_component::notification::Notification::error(error)
                    .autohide(false)
                    .action(move |_, window, cx| {
                        let text = text.clone();
                        let copied = window.use_keyed_state("copied-error", cx, |_, _| false);
                        let done = *copied.read(cx);
                        Button::new("copy-error")
                            .ghost()
                            .xsmall()
                            .icon(Icon::new(if done {
                                IconName::Check
                            } else {
                                IconName::Copy
                            }))
                            .tooltip("Copy message")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                copied.update(cx, |c, cx| {
                                    *c = true;
                                    cx.notify();
                                });
                                let copied = copied.clone();
                                cx.spawn(async move |cx| {
                                    cx.background_executor()
                                        .timer(std::time::Duration::from_secs(2))
                                        .await;
                                    let _ = copied.update(cx, |c, cx| {
                                        *c = false;
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })
                    }),
                cx,
            );
        }
        let active = self.store.active;
        let conversation = &self.store.conversations[active];
        // Only the session the reply was sent from shows the live row.
        let streaming_here = self.busy && self.streaming_id == Some(conversation.id);

        let body: AnyElement = if self.page != Page::Chat {
            self.page_view(self.page, cx).into_any_element()
        } else if conversation.messages.is_empty() && !streaming_here {
            self.empty_state(cx).into_any_element()
        } else {
            let mut rows: Vec<AnyElement> = Vec::new();
            for (i, m) in conversation.messages.iter().enumerate() {
                rows.push(self.message_row(i, m, false, window, cx).into_any_element());
            }
            if streaming_here && self.pending.is_none() {
                let live = Message {
                    role: "assistant".into(),
                    content: self.partial.clone(),
                    model: self.store.model.clone(),
                    reasoning: self.partial_reasoning.clone(),
                    tokens: self.live_tokens,
                    per_second: self.live_per_second,
                    ..Default::default()
                };
                rows.push(
                    self.message_row(usize::MAX, &live, true, window, cx)
                        .into_any_element(),
                );
            }
            if streaming_here {
                if let Some(card) = self.approval_card(cx) {
                    rows.push(card);
                }
            }
            // The scrollbar must sit outside the scrolling element, or it
            // scrolls away with the content.
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("messages")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .child(
                            v_flex()
                                .w_full()
                                .when(self.sidebar_open, |d| d.max_w(relative(0.82)))
                                .mx_auto()
                                .px_6()
                                .py_6()
                                .gap_6()
                                .children(rows),
                        ),
                )
                .vertical_scrollbar(&self.scroll)
                .into_any_element()
        };

        let main = v_flex()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(body)
            .when(self.page == Page::Chat, |this| {
                this.child(
                    div()
                        .w_full()
                        .when(self.sidebar_open, |d| d.max_w(relative(0.82)))
                        .mx_auto()
                        .px_6()
                        .pb_2()
                        .child(self.composer(cx)),
                )
            })
            .child(self.status_bar(cx))
            .children(Root::render_notification_layer(window, cx));

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
            .children(self.settings_overlay(window, cx))
            .children(Root::render_dialog_layer(window, cx))
    }
}
