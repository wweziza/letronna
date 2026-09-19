//! The card the chat shows when a tool needs the user: approve a write or a
//! command, or answer an `ask_user` question.

use crate::core::tools;
use crate::prelude::*;

impl Chat {
    pub(crate) fn submit_answer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.answer.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.answer
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.resolve_call(Verdict::Answer(text), cx);
    }

    /// The row under the messages while tools run: a progress line, or the
    /// card waiting on the user. `None` when nothing is queued.
    pub(crate) fn approval_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pending = self.pending.as_ref()?;
        let call = pending.calls.front()?;
        let ctx = tools::ToolContext {
            workspace: pending.connection.workspace.clone(),
        };
        let summary = tools::summary(&call.name, &call.arguments);
        let gated = tools::needs_approval(&call.name)
            && (call.name == tools::ASK_USER
                || !(self.store.skip_approvals || self.session_allow.contains(&call.name)));
        if !gated {
            return Some(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        gpui_component::spinner::Spinner::new()
                            .xsmall()
                            .color(cx.theme().primary),
                    )
                    .child(call.name.clone())
                    .child(div().font_family(MONO_FONT).child(summary))
                    .into_any_element(),
            );
        }

        let args = tools::parse_args(&call.arguments).unwrap_or_default();
        let card = v_flex()
            .w_full()
            .gap_3()
            .p_3()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().primary.opacity(0.5))
            .bg(cx.theme().secondary.opacity(0.3));

        if call.name == tools::ASK_USER {
            let question = args["question"].as_str().unwrap_or_default().to_owned();
            let options: Vec<String> = args["options"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let mut chips = h_flex().flex_wrap().gap_2();
            for (i, option) in options.into_iter().enumerate() {
                let pick = option.clone();
                chips = chips.child(
                    Button::new(("option", i))
                        .outline()
                        .small()
                        .label(option)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.resolve_call(Verdict::Answer(pick.clone()), cx)
                        })),
                );
            }
            return Some(
                card.child(div().text_sm().child(question))
                    .child(chips)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.answer).small().w_full()),
                            )
                            .child(
                                Button::new("answer")
                                    .primary()
                                    .small()
                                    .label("Reply")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.submit_answer(window, cx)
                                    })),
                            ),
                    )
                    .into_any_element(),
            );
        }

        let title = match call.name.as_str() {
            "write_file" => format!("Letronna wants to write {summary}"),
            "edit_file" => format!("Letronna wants to edit {summary}"),
            "run_command" => "Letronna wants to run a command".to_owned(),
            other => format!("Letronna wants to use {other}"),
        };
        let preview = tools::preview(&call.name, &call.arguments, &ctx);
        Some(
            card.child(
                div()
                    .text_sm()
                    .font_family(HEADING_FONT)
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                // Short box with its own scrollbar, so a long file does not
                // stretch the chat.
                div()
                    .id("preview")
                    .occlude()
                    .h(px(180.))
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().background)
                    .child(
                        div()
                            .p_2()
                            .text_xs()
                            .font_family(MONO_FONT)
                            .line_height(px(18.))
                            .whitespace_normal()
                            .child(tools::clamp_lines(&preview, 400))
                            .overflow_y_scrollbar(),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("allow")
                            .primary()
                            .small()
                            .label("Allow")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.resolve_call(Verdict::Allow { remember: false }, cx)
                            })),
                    )
                    .child(
                        Button::new("allow-session")
                            .outline()
                            .small()
                            .label("Allow for this session")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.resolve_call(Verdict::Allow { remember: true }, cx)
                            })),
                    )
                    .child(Button::new("deny").ghost().small().label("Deny").on_click(
                        cx.listener(|this, _, _, cx| this.resolve_call(Verdict::Deny, cx)),
                    )),
            )
            .into_any_element(),
        )
    }
}
