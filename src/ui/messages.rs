use crate::prelude::*;

impl Chat {
    pub(crate) fn message_row(
        &self,
        index: usize,
        message: &Message,
        streaming: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if message.role == "tool" {
            return self.tool_row(index, message, cx).into_any_element();
        }
        let is_user = message.role == "user";
        let label: String = if is_user {
            "You".into()
        } else if message.model.is_empty() {
            self.store.model.clone()
        } else {
            message.model.clone()
        };
        let leading = px(26.);
        let cx_muted = cx.theme().muted_foreground;
        let body: AnyElement = if is_user {
            TextView::markdown(("md", index), message.content.clone(), window, cx)
                .text(message.content.clone())
                .selectable(true)
                .line_height(leading)
                .into_any_element()
        } else if message.content.is_empty() && !message.tool_calls.is_empty() {
            // A turn that only asked for tools; the tool rows below say what happened.
            div().into_any_element()
        } else if message.content.is_empty() {
            h_flex()
                .gap_2()
                .items_center()
                .text_color(cx.theme().muted_foreground)
                .child(
                    gpui_component::spinner::Spinner::new()
                        .xsmall()
                        .color(cx.theme().primary),
                )
                .child("Thinking…")
                .into_any_element()
        } else {
            TextView::markdown(("md", index), message.content.clone(), window, cx)
                .selectable(true)
                .line_height(leading)
                .style(gpui_component::text::TextViewStyle::default().heading_gap(gpui::rems(1.)))
                .code_block_actions(move |block, _, _| {
                    let code = block.code();
                    let mut hasher = std::hash::DefaultHasher::new();
                    std::hash::Hash::hash(&code, &mut hasher);
                    let id = std::hash::Hasher::finish(&hasher);
                    let lang = block.lang().unwrap_or_default();
                    h_flex()
                        .gap_2()
                        .items_center()
                        .when(!lang.is_empty(), |this| {
                            this.child(div().text_xs().text_color(cx_muted).child(lang.to_string()))
                        })
                        .child(
                            gpui_component::clipboard::Clipboard::new(SharedString::from(format!(
                                "copy-{index}-{id}"
                            )))
                            .value(code),
                        )
                })
                .into_any_element()
        };
        v_flex()
            .w_full()
            .gap_2p5()
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
                        this.child(
                            div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(cx.theme().primary)
                                .with_animation(
                                    "pulse",
                                    Animation::new(std::time::Duration::from_millis(1000))
                                        .repeat()
                                        .with_easing(ease_in_out),
                                    |dot, delta| {
                                        let wave = 1.0 - (delta * 2.0 - 1.0).abs();
                                        dot.opacity(0.25 + 0.75 * wave)
                                    },
                                ),
                        )
                    }),
            )
            .when(!is_user && (!message.reasoning.is_empty()), |this| {
                this.child(self.thinking_block(index, message, streaming, window, cx))
            })
            .child(div().pb_1().child(body))
            .when(
                !is_user && message.tokens > 0 && !message.content.is_empty(),
                |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} tokens · {:.1} tok/s",
                                message.tokens, message.per_second
                            )),
                    )
                },
            )
            .with_animation(
                ("appear", index),
                Animation::new(std::time::Duration::from_millis(260))
                    .with_easing(gpui::ease_out_quint()),
                |el, delta| el.opacity(delta).mt(px(10. * (1. - delta))),
            )
            .into_any_element()
    }

    /// A tool result: one quiet line, click to see the output.
    fn tool_row(
        &self,
        index: usize,
        message: &Message,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = self.thinking_open.contains(&index);
        let summary = crate::core::tools::summary(&message.name, &message.args);
        let lines = message.content.lines().count();
        let content = message.content.clone();
        let (status, color) = if message.is_error {
            (message.content.clone(), cx.theme().danger)
        } else {
            (format!("{lines} lines"), cx.theme().muted_foreground)
        };
        v_flex()
            .w_full()
            .gap_1()
            .child(
                h_flex()
                    .id(("tool", index))
                    .h(px(26.))
                    .px_2()
                    .gap_1p5()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .text_xs()
                    .font_family(HEADING_FONT)
                    .text_color(cx.theme().muted_foreground)
                    .hover(|s| s.bg(cx.theme().secondary.opacity(0.5)))
                    .child(
                        Icon::new(if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size_3(),
                    )
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(message.name.clone()),
                    )
                    .child(div().font_family(MONO_FONT).child(summary))
                    .child(div().text_color(color).truncate().child(status))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.thinking_open.remove(&index) {
                            this.thinking_open.insert(index);
                        }
                        cx.notify();
                    })),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .id(("tool-out", index))
                        .ml_2()
                        .pl_3()
                        .max_h(px(320.))
                        .overflow_y_scroll()
                        .border_l_2()
                        .border_color(cx.theme().border)
                        .text_xs()
                        .font_family(MONO_FONT)
                        .line_height(px(18.))
                        .text_color(cx.theme().muted_foreground)
                        .whitespace_normal()
                        .child(content),
                )
            })
    }

    fn thinking_block(
        &self,
        index: usize,
        message: &Message,
        streaming: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let open = streaming || self.thinking_open.contains(&index);
        let reasoning = message.reasoning.clone();
        v_flex()
            .w_full()
            .gap_1()
            .child(
                h_flex()
                    .id(("think", index))
                    .h(px(26.))
                    .px_2()
                    .gap_1p5()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .cursor_pointer()
                    .text_xs()
                    .font_family(HEADING_FONT)
                    .text_color(cx.theme().muted_foreground)
                    .hover(|s| s.bg(cx.theme().secondary.opacity(0.5)))
                    .child(
                        Icon::new(if open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size_3(),
                    )
                    .child(if streaming {
                        "Thinking…"
                    } else {
                        "Thought process"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.thinking_open.contains(&index) {
                            this.thinking_open.remove(&index);
                        } else {
                            this.thinking_open.insert(index);
                        }
                        cx.notify();
                    })),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .ml_2()
                        .pl_3()
                        .border_l_2()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            TextView::markdown(("reason", index), reasoning, window, cx)
                                .selectable(true)
                                .line_height(px(21.)),
                        ),
                )
            })
    }

    pub(crate) fn empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(crate) fn page_view(&self, page: Page, cx: &mut Context<Self>) -> impl IntoElement {
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
}
