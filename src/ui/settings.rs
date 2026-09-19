use crate::prelude::*;

type ApplyMode = std::rc::Rc<dyn Fn(plugins::PresenceMode, &mut App)>;

const PAGES: &[(&str, AppIcon)] = &[
    ("Gateways", AppIcon::Plugs),
    ("Model", AppIcon::Robot),
    ("Appearance", AppIcon::Palette),
    ("Privacy", AppIcon::Shield),
    ("Plugins", AppIcon::Puzzle),
    ("About", AppIcon::Info),
];

impl Chat {
    pub(crate) fn select_gateway(
        &mut self,
        name: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let key = self.keys.get(key_slot(name)).cloned().unwrap_or_default();
        self.key
            .update(cx, |input, cx| input.set_value(key, window, cx));
        cx.notify();
    }

    pub(crate) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.store.endpoint = self.endpoint.read(cx).value().to_string();
        self.store.model = self.model.read(cx).value().to_string();
        let key = self.key.read(cx).value().to_string();
        let slot = key_slot(&self.store.gateway).to_string();
        if key.is_empty() {
            self.keys.remove(&slot);
        } else {
            self.keys.insert(slot, key);
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
        self.close_settings(cx);
    }

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Snapshot the window before the overlay covers it.
        #[cfg(windows)]
        {
            self.settings_backdrop = crate::platform::backdrop::capture(window);
        }
        #[cfg(not(windows))]
        let _ = window;
        self.settings_open = true;
        self.settings_closing = false;
        cx.notify();
    }

    pub(crate) fn close_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings_closing {
            return;
        }
        self.settings_closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(170))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.settings_open = false;
                this.settings_closing = false;
                this.settings_backdrop = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// The settings overlay, mounted while open so both open and close animate.
    pub(crate) fn settings_overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.settings_open {
            return None;
        }
        let closing = self.settings_closing;
        let anim = || {
            Animation::new(std::time::Duration::from_millis(180))
                .with_easing(gpui::ease_out_quint())
        };
        let blur = self.settings_backdrop.clone().map(|image| {
            img(image)
                .absolute()
                .size_full()
                .object_fit(ObjectFit::Fill)
                .with_animation(
                    if closing { "blur-out" } else { "blur-in" },
                    anim(),
                    move |el, delta| el.opacity(if closing { 1. - delta } else { delta }),
                )
        });
        let scrim = div()
            .id("settings-scrim")
            .absolute()
            .size_full()
            .bg(gpui::black())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_settings(cx)),
            )
            .with_animation(
                if closing { "scrim-out" } else { "scrim-in" },
                anim(),
                move |el, delta| {
                    el.opacity(if closing {
                        0.18 * (1. - delta)
                    } else {
                        0.18 * delta
                    })
                },
            );
        let panel = self
            .settings_panel(window, cx)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .with_animation(
                if closing { "panel-out" } else { "panel-in" },
                anim(),
                move |el, delta| {
                    let p = if closing { 1. - delta } else { delta };
                    el.opacity(p).mt(px(-14. * (1. - p)))
                },
            );
        Some(
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .children(blur)
                .child(scrim)
                .child(panel)
                .into_any_element(),
        )
    }

    fn settings_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        let key = self.key.clone();
        let chat = cx.entity();
        let pick = self.gateway_pick.clone();
        let presence = self.presence_pick.clone();
        let page = self.settings_page.clone();
        let vp = window.viewport_size();
        let gap = px(44.);
        let height = vp.height - gap * 2.;
        let current_page = *page.read(cx);
        let nav = PAGES.iter().fold(
            v_flex()
                .w(px(210.))
                .h_full()
                .flex_shrink_0()
                .gap_0p5()
                .p_3()
                .bg(cx.theme().sidebar)
                .border_r_1()
                .border_color(cx.theme().border),
            |nav, (name, icon)| {
                let page = page.clone();
                let active = *name == current_page;
                nav.child(
                    h_flex()
                        .id(*name)
                        .h(px(30.))
                        .px_2()
                        .gap_2()
                        .items_center()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .text_sm()
                        .font_family(HEADING_FONT)
                        .font_weight(FontWeight::MEDIUM)
                        .when(active, |this| this.bg(cx.theme().sidebar_accent))
                        .hover(|s| s.bg(cx.theme().sidebar_accent.opacity(0.6)))
                        .child(
                            Icon::new(*icon)
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(*name)
                        .on_click(move |_, _, cx| {
                            page.update(cx, |p, cx| {
                                *p = name;
                                cx.notify();
                            })
                        }),
                )
            },
        );
        let body: AnyElement = match current_page {
            "Model" => model_page(&model, &chat, cx).into_any_element(),
            "Appearance" => text_page(
                "Appearance",
                "Dark theme with the Letronna palette. Theme and font options land here later.",
                cx,
            )
            .into_any_element(),
            "Privacy" => privacy_page(&chat, &presence, cx).into_any_element(),
            "Plugins" => plugins_page(&chat, cx).into_any_element(),
            "About" => about_page(cx).into_any_element(),
            _ => gateway_page(&pick, &endpoint, &key, &chat, cx).into_any_element(),
        };
        div()
            .w(vp.width - gap * 2.)
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius_lg)
            .shadow_lg()
            .child(
                h_flex()
                    .h(height)
                    .items_start()
                    .overflow_hidden()
                    .rounded(cx.theme().radius_lg)
                    .child(nav)
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                div()
                                    .id("settings-body")
                                    .size_full()
                                    .overflow_y_scroll()
                                    .px_8()
                                    .py_6()
                                    .child(
                                        div().child(body).with_animation(
                                            SharedString::from(format!("page-{current_page}")),
                                            Animation::new(std::time::Duration::from_millis(200))
                                                .with_easing(gpui::ease_out_quint()),
                                            |el, delta| el.opacity(delta).mt(px(6. * (1. - delta))),
                                        ),
                                    ),
                            )
                            .child(
                                div().absolute().top_3().right_3().child(
                                    Button::new("close-settings")
                                        .ghost()
                                        .xsmall()
                                        .icon(Icon::new(IconName::Close))
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.close_settings(cx)),
                                        ),
                                ),
                            ),
                    ),
            )
    }
}

fn plugins_page(chat: &Entity<Chat>, cx: &App) -> Div {
    let mut list = v_flex().gap_2();
    for info in plugins::list(cx) {
        let chat = chat.clone();
        let id = info.id;
        let enabled = info.enabled;
        let row = h_flex()
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(info.name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .px_1p5()
                                    .rounded(cx.theme().radius)
                                    .bg(cx.theme().secondary)
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if info.builtin { "Built in" } else { "External" }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(info.description),
                    ),
            )
            .child(
                div().cursor_pointer().child(
                    gpui_component::switch::Switch::new(SharedString::from(format!("plugin-{id}")))
                        .checked(enabled)
                        .on_click(move |_, _, cx| {
                            plugins::set_enabled(cx, id, !enabled);
                            chat.update(cx, |_, cx| cx.notify());
                        }),
                ),
            );
        let settings = if enabled {
            plugins::render_settings(cx, id)
        } else {
            None
        };
        list = list.child(
            v_flex()
                .gap_2()
                .p_3()
                .rounded(cx.theme().radius)
                .bg(cx.theme().secondary.opacity(0.3))
                .child(row)
                .children(settings),
        );
    }
    v_flex()
        .gap_4()
        .child(heading(
            "Plugins",
            "Optional integrations. Toggle one off to unload it. Choices are saved in plugins.json.",
            cx,
        ))
        .child(list)
}

fn heading(title: &'static str, blurb: &'static str, cx: &App) -> Div {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_family(HEADING_FONT)
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(blurb),
        )
}

fn label(text: &'static str, cx: &App) -> Div {
    div()
        .text_xs()
        .font_family(HEADING_FONT)
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn privacy_page(chat: &Entity<Chat>, presence: &Entity<String>, cx: &App) -> Div {
    let current = plugins::PresenceMode::from_str(presence.read(cx));
    let enabled = plugins::list(cx)
        .iter()
        .find(|p| p.id == plugins::DISCORD)
        .map(|p| p.enabled)
        .unwrap_or(true);

    let apply: ApplyMode = {
        let chat = chat.clone();
        let presence = presence.clone();
        std::rc::Rc::new(move |mode: plugins::PresenceMode, cx: &mut App| {
            presence.update(cx, |p, cx| {
                *p = mode.as_str().into();
                cx.notify();
            });
            plugins::emit(cx, plugins::AppEvent::PresenceMode(mode));
            chat.update(cx, |this, cx| {
                this.store.presence = mode.as_str().into();
                this.save();
                cx.notify();
            });
        })
    };

    let level_label = if current == plugins::PresenceMode::Minimal {
        "Hide activity details"
    } else {
        "Show what I am doing"
    };
    let detailed = apply.clone();
    let minimal = apply.clone();
    let level = h_flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2p5()
        .rounded(cx.theme().radius)
        .bg(cx.theme().secondary.opacity(0.3))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("What to share"),
        )
        .child(
            Button::new("presence-level")
                .ghost()
                .small()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(level_label)
                        .child(Icon::new(IconName::ChevronDown).size_3()),
                )
                .dropdown_menu(move |menu, _, _| {
                    let detailed = detailed.clone();
                    let minimal = minimal.clone();
                    menu.min_w(px(220.))
                        .item(
                            PopupMenuItem::new("Show what I am doing")
                                .checked(current == plugins::PresenceMode::Detailed)
                                .on_click(move |_, _, cx| {
                                    detailed(plugins::PresenceMode::Detailed, cx)
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Hide activity details")
                                .checked(current == plugins::PresenceMode::Minimal)
                                .on_click(move |_, _, cx| {
                                    minimal(plugins::PresenceMode::Minimal, cx)
                                }),
                        )
                }),
        );

    v_flex()
        .gap_4()
        .child(heading(
            "Discord Rich Presence",
            "What Letronna shares with Discord. The integration is turned on or off in the Plugins tab.",
            cx,
        ))
        .child(if enabled {
            level.into_any_element()
        } else {
            div()
                .p_3()
                .rounded(cx.theme().radius)
                .bg(cx.theme().secondary.opacity(0.3))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Discord Rich Presence is off. Enable it in the Plugins tab to choose what to share.")
                .into_any_element()
        })
}

fn about_page(cx: &App) -> Div {
    let muted = cx.theme().muted_foreground;
    let line =
        |text: &'static str, size: f32| div().text_size(px(size)).text_color(muted).child(text);
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .text_center()
        .child(
            img("images/letronna.png")
                .size(px(150.))
                .rounded(cx.theme().radius_lg)
                .mb_3(),
        )
        .child(
            div()
                .font_family(WORDMARK_FONT)
                .italic()
                .text_size(px(36.))
                .child(BRAND),
        )
        .child(line(
            concat!(
                "Version ",
                env!("CARGO_PKG_VERSION"),
                "  ·  build ",
                env!("GIT_HASH")
            ),
            13.,
        ))
        .child(
            div()
                .text_sm()
                .pt_3()
                .child("A small native desktop agent. Rust, GPU rendered, no browser inside."),
        )
        .child(line("Developed by the Gadlus Engineering team.", 13.))
        .child(
            v_flex()
                .pt_6()
                .gap_1()
                .items_center()
                .child(line(
                    "© 2026 Gadlus Engineering. Released under the MIT License.",
                    12.,
                ))
                .child(line(
                    "Built with GPUI and gpui-component (Apache-2.0). Icons by Phosphor (MIT).",
                    12.,
                )),
        )
}

fn text_page(title: &'static str, blurb: &'static str, cx: &App) -> Div {
    v_flex().gap_4().child(heading(title, blurb, cx))
}

fn model_page(model: &Entity<InputState>, chat: &Entity<Chat>, cx: &App) -> Div {
    let save = chat.clone();
    v_flex()
        .gap_4()
        .child(heading(
            "Model",
            "The model used for new messages. Pick from the composer, or type an id here.",
            cx,
        ))
        .child(
            v_flex()
                .gap_1()
                .child(label("Default model", cx))
                .child(Input::new(model)),
        )
        .child(
            h_flex().justify_end().child(
                Button::new("save-model")
                    .primary()
                    .small()
                    .label("Save")
                    .on_click(move |_, window, cx| {
                        save.update(cx, |this, cx| this.apply_settings(window, cx))
                    }),
            ),
        )
}

fn gateway_page(
    pick: &Entity<String>,
    endpoint: &Entity<InputState>,
    key: &Entity<InputState>,
    chat: &Entity<Chat>,
    cx: &App,
) -> Div {
    let current = pick.read(cx).clone();
    let g = gateway(&current);
    let mut cards = v_flex().gap_3();
    for row in GATEWAYS.chunks(3) {
        let mut line = h_flex().gap_3();
        for gw in row {
            let chat = chat.clone();
            let active = gw.name == current;
            line = line.child(
                v_flex()
                    .id(gw.name)
                    .flex_1()
                    .w_0()
                    .h(px(120.))
                    .p_3()
                    .gap_1p5()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(if active {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(cx.theme().secondary.opacity(0.3))
                    .cursor_pointer()
                    .hover(|s| s.border_color(cx.theme().primary.opacity(0.6)))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .font_family(HEADING_FONT)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(gw.name),
                            )
                            .when(active, |this| {
                                this.child(
                                    Icon::new(IconName::Check)
                                        .size_4()
                                        .text_color(cx.theme().primary),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(gw.blurb),
                    )
                    .on_click(move |_, window, cx| {
                        chat.update(cx, |this, cx| this.select_gateway(gw.name, window, cx))
                    }),
            );
        }
        cards = cards.child(line);
    }
    let save = chat.clone();
    v_flex()
        .gap_5()
        .child(heading(
            "Gateway Connection",
            "Where messages are sent. Every gateway speaks the OpenAI chat format. Keys are stored per gateway in keys.json on this machine.",
            cx,
        ))
        .child(v_flex().gap_2().child(label("Gateway", cx)).child(cards))
        .child(
            v_flex()
                .gap_1()
                .child(label("Base URL", cx))
                .child(Input::new(endpoint)),
        )
        .child(
            v_flex()
                .gap_1()
                .child(label(
                    if g.needs_key {
                        "API key (required)"
                    } else {
                        "API key (optional)"
                    },
                    cx,
                ))
                .child(Input::new(key)),
        )
        .child(
            h_flex().justify_end().child(
                Button::new("save-gateway")
                    .primary()
                    .small()
                    .label("Save and reconnect")
                    .on_click(move |_, window, cx| {
                        save.update(cx, |this, cx| this.apply_settings(window, cx))
                    }),
            ),
        )
}
