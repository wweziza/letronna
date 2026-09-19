use crate::prelude::*;

impl Chat {
    pub(crate) fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .flex_1()
                .items_center()
                .gap_2()
                .child(
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
                )
                .child(div().flex_1())
                .child(self.account_chip(cx)),
        )
    }

    fn account_chip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // No native sign-in yet: everyone is a guest. The menu is the shell that
        // becomes Profile / Settings / Log out once accounts land.
        let name = "Guest";
        let chat = cx.entity();
        Button::new("account")
            .ghost()
            .xsmall()
            .compact()
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Icon::new(IconName::CircleUser)
                            .size_4()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(name),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .dropdown_menu_with_anchor(gpui::Corner::TopRight, move |menu, _, _| {
                let settings = chat.clone();
                menu.min_w(px(180.))
                    .item(
                        PopupMenuItem::new("Profile")
                            .icon(Icon::new(IconName::CircleUser))
                            .disabled(true),
                    )
                    .item(
                        PopupMenuItem::new("Settings")
                            .icon(Icon::new(IconName::Settings))
                            .on_click(move |_, window, cx| {
                                settings.update(cx, |this, cx| this.open_settings(window, cx))
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Sign in")
                            .icon(Icon::new(IconName::CircleUser))
                            .on_click(|_, _, cx| cx.open_url("https://chat.aile.sh")),
                    )
            })
    }

    fn gateway_chip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (dot, state, tip) = if self.loading_models {
            (
                cx.theme().muted_foreground,
                "checking",
                format!("{}: loading model list", self.store.gateway),
            )
        } else if let Some(e) = &self.gateway_error {
            (
                cx.theme().danger,
                "error",
                format!("{}: {e}", self.store.gateway),
            )
        } else {
            (
                cx.theme().green,
                "ready",
                format!("{}: {} models", self.store.gateway, self.models.len()),
            )
        };
        Button::new("gateway")
            .ghost()
            .xsmall()
            .text_color(cx.theme().muted_foreground)
            .tooltip(tip)
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(div().size(px(6.)).rounded_full().bg(dot))
                    .child(div().text_color(cx.theme().foreground).child("Gateway"))
                    .child(state),
            )
            .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx)))
    }

    /// The session's project folder. Click to pick or change it.
    fn workspace_chip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = &self.store.conversations[self.store.active].workspace;
        let (name, tip) = match workspace {
            Some(p) => (
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.display().to_string()),
                p.display().to_string(),
            ),
            None => ("No project".into(), "Attach a project folder".into()),
        };
        Button::new("workspace")
            .ghost()
            .xsmall()
            .text_color(cx.theme().muted_foreground)
            .tooltip(tip)
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(Icon::new(AppIcon::Folder).size_3())
                    .child(name),
            )
            .on_click(cx.listener(|this, _, window, cx| this.attach_workspace(window, cx)))
    }

    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h(px(20.))
            .px_3()
            .gap_2()
            .items_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(self.gateway_chip(cx))
            .child(self.workspace_chip(cx))
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
