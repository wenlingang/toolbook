//! Toolbook's macOS app: the Rust side of the window.

mod blocks;
pub mod bridge;
pub mod editor;
pub mod highlight;
pub mod install;
pub mod library;
pub mod markdown_view;
mod memos;
mod runner;
mod secrets;
pub mod share;
mod slash;
pub mod themes;
pub mod update;
pub mod updater;

use nsh_core::prelude::*;

/// The crate's UI set (`nsh.json` `"rust"`).
#[native_module("toolbook/native")]
fn native(registry: &mut Registry) {
    registry
        .register::<editor::NoteEditor>()
        .register::<markdown_view::MarkdownView>()
        .register::<share::PreviewView>()
        .module::<themes::Themes>()
        .module::<library::Library>()
        .module::<share::Share>()
        .module::<updater::Updater>();
}
