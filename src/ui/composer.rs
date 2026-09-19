use crate::prelude::*;

impl Chat {
    pub(crate) fn model_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(crate) fn attach(
        &mut self,
        directories: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    /// Pick the project folder for the active session. Tools resolve paths here.
    pub(crate) fn attach_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        let id = self.store.conversations[self.store.active].id;
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let _ = this.update(cx, |this, cx| {
                    if let Some(ix) = this.store.index_of(id) {
                        this.store.conversations[ix].workspace = paths.into_iter().next();
                        this.save();
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn insert(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |input, cx| {
            let mut value = input.value().to_string();
            value.push_str(text);
            input.set_value(value, window, cx);
        });
    }

    pub(crate) fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                        PopupMenuItem::new("Project folder")
                            .icon(Icon::new(AppIcon::Folder))
                            .on_click(move |_, window, cx| {
                                folder.update(cx, |this, cx| this.attach_workspace(window, cx))
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
}
