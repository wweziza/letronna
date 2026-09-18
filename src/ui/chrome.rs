use crate::prelude::*;

impl Chat {
    pub(crate) fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
