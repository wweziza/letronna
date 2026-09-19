pub(crate) mod approval;
pub(crate) mod chrome;
pub(crate) mod composer;
pub(crate) mod messages;
pub(crate) mod sessions;
pub(crate) mod settings;
pub(crate) mod sidebar;

use crate::prelude::*;

const CODE_LINE: Pixels = px(18.);

/// Monospaced output that turns into a scroll box only once it is taller than
/// `max`. A short command then takes the room it needs instead of reserving a
/// tall panel, and a long one clips instead of painting over the chat.
///
/// Every block needs its own scroll state, keyed on `id`. The convenience
/// `overflow_y_scrollbar()` keys on the call site instead, so all of them
/// would share one handle and fight over it.
pub(crate) fn code_block(
    id: impl Into<ElementId>,
    text: String,
    max: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let id = id.into();
    let body = div()
        .text_xs()
        .font_family(MONO_FONT)
        .line_height(CODE_LINE)
        .whitespace_normal()
        .child(text.clone());
    if CODE_LINE * (text.lines().count() as f32) <= max {
        return body.into_any_element();
    }
    let scroll = window
        .use_keyed_state(id.clone(), cx, |_, _| ScrollHandle::default())
        .read(cx)
        .clone();
    div()
        .id(id)
        .relative()
        // A definite height, so the scroll area inside has something to
        // resolve `size_full` against.
        .h(max)
        .child(
            div()
                .id("code-scroll")
                .occlude()
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .child(body),
        )
        .vertical_scrollbar(&scroll)
        .into_any_element()
}
