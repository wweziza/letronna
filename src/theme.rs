use crate::prelude::*;

pub(crate) const HEADING_FONT: &str = "Segoe UI Variable Display";
pub(crate) const BODY_FONT: &str = "Segoe UI Variable Text";
pub(crate) const WORDMARK_FONT: &str = "Georgia";

pub(crate) fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let t = Theme::global_mut(cx);
    let blue: Hsla = rgb(0x3d6bff).into();
    let bg: Hsla = rgb(0x0c0f15).into();
    let side: Hsla = rgb(0x090c11).into();
    t.primary = blue;
    t.primary_hover = rgb(0x5580ff).into();
    t.primary_active = rgb(0x2f5ae6).into();
    t.ring = blue;
    t.link = blue;
    t.caret = blue;
    t.selection = blue.opacity(0.35);
    t.background = bg;
    t.foreground = rgb(0xd4d8e0).into();
    t.sidebar_foreground = rgb(0xc6cbd5).into();
    t.muted_foreground = rgb(0x7c8493).into();
    t.sidebar = side;
    t.title_bar = side;
    t.title_bar_border = side;
    t.sidebar_accent = rgb(0x151a23).into();
    t.sidebar_border = rgb(0x11151c).into();
    t.secondary = rgb(0x171c25).into();
    t.secondary_hover = rgb(0x1e2430).into();
    t.border = rgb(0x1c2230).into();
    t.input = rgb(0x1c2230).into();
    t.radius = px(4.);
    t.radius_lg = px(6.);
    t.font_family = BODY_FONT.into();
}
