use crate::prelude::*;

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
            let mut list = v_flex().w(px(160.)).flex_shrink_0().gap_0p5().pr_4();
            for gw in GATEWAYS {
                let chat = chat.clone();
                let active = gw.name == current;
                list = list.child(
                    Button::new(gw.name)
                        .small()
                        .w_full()
                        .justify_start()
                        .map(|b| if active { b.primary() } else { b.ghost() })
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
                    h_flex().items_start().py_2().child(list).child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_4()
                        .child(div().text_sm().font_family(HEADING_FONT).font_weight(FontWeight::MEDIUM).child(g.name))
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
                ))
        });
    }
}
