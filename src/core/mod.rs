//! Domain layer: the chat transport, gateway presets, and on-disk state.
//! No UI, no OS specifics. This is the part that could run headless.

pub(crate) mod chat;
pub(crate) mod gateway;
pub(crate) mod store;
