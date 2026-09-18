use crate::prelude::*;

const PAGES: &[(&str, AppIcon)] = &[
    ("Gateways", AppIcon::Plugs),
    ("Model", AppIcon::Robot),
    ("Appearance", AppIcon::Palette),
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
        let key = self.keys.get(name).cloned().unwrap_or_default();
        self.key
            .update(cx, |input, cx| input.set_value(key, window, cx));
        cx.notify();
    }

    pub(crate) fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        let key = self.key.clone();
        let chat = cx.entity();
        let pick = self.gateway_pick.clone();
        let page = self.settings_page.clone();
        window.open_dialog(cx, move |dialog, window, cx| {
            let size = window.viewport_size();
            let current_page = *page.read(cx);
            let nav = PAGES.iter().fold(
                v_flex().w(px(190.)).flex_shrink_0().gap_0p5().pr_4(),
                |nav, (name, icon)| {
                    let page = page.clone();
                    let active = *name == current_page;
                    nav.child(
                        Button::new(*name)
                            .small()
                            .w_full()
                            .justify_start()
                            .map(|b| if active { b.primary() } else { b.ghost() })
                            .icon(Icon::new(*icon))
                            .label(*name)
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
                "About" => text_page(
                    "About",
                    concat!(
                        "Letronna v",
                        env!("CARGO_PKG_VERSION"),
                        " (",
                        env!("GIT_HASH"),
                        "). A small native desktop agent built with GPUI. MIT licensed."
                    ),
                    cx,
                )
                .into_any_element(),
                _ => gateway_page(&pick, &endpoint, &key, &chat, cx).into_any_element(),
            };
            dialog
                .title(div().font_family(HEADING_FONT).child("Settings"))
                .w(size.width * 0.9)
                .child(
                    h_flex()
                        .items_start()
                        .h(size.height * 0.72)
                        .child(nav)
                        .child(
                            div()
                                .id("settings-body")
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .overflow_y_scroll()
                                .pl_6()
                                .border_l_1()
                                .border_color(cx.theme().border)
                                .child(body),
                        ),
                )
        });
    }
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
    let mut cards = h_flex().flex_wrap().gap_3();
    for gw in GATEWAYS {
        let chat = chat.clone();
        let active = gw.name == current;
        cards = cards.child(
            v_flex()
                .id(gw.name)
                .w(px(190.))
                .h(px(112.))
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
