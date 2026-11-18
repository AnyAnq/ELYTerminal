//! The terminal assistant: model connections, conversations and one running task.

pub mod config;
mod connection;
mod credentials;
#[cfg(test)]
mod fixture;
mod panel;
mod policy;
mod protocol;
pub mod settings;
mod tools;

pub use connection::Cancel;
pub use panel::{Assistant, Notice, Target};

pub const CALL_LIMIT: usize = 30;

#[derive(Default)]
struct Active(Option<Cancel>);
impl gpui::Global for Active {}

pub fn init(cx: &mut gpui::App) {
    cx.set_global(Active::default());
}

pub fn word(cx: &gpui::App, key: &str) -> gpui::SharedString {
    ely_gpui_component::i18n::text(cx, key, &[])
}
