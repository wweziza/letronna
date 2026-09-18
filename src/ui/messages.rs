use crate::prelude::*;

impl Chat {
    pub(crate) fn message_row(
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
                .code_block_actions(|block, _, _| {
                    let code = block.code();
                    Button::new("copy-code")
                        .ghost()
                        .xsmall()
                        .icon(Icon::new(IconName::Copy))
                        .tooltip("Copy code")
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()))
                        })
                })
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
