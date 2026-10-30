//! The `/` menu: typing `/` at the start of a line lists the tools, and
//! choosing one puts an empty tool block there.

use std::sync::Arc;

use nsh_base::rich_editor::commands::tiptap::{
    InsertContentOptions, insert_content_at, set_text_selection,
};
use nsh_base::rich_editor::extensions::suggestion::{
    SuggestionOptions, SuggestionRange, SuggestionTrigger, suggestion, suggestion_state,
};
use nsh_base::rich_editor::model::Fragment;
use nsh_base::rich_editor::state::Plugin;
use nsh_base::rich_editor::view::EditorSession;
use serde_json::{Value as Json, json};

/// The plugin's key.
pub(crate) const KEY: &str = "slash";

/// The tools the menu offers: fence name and what people call it.
pub(crate) use toolbook_core::doc::TOOL_LABELS as TOOLS;

/// The tools matching `query` (by fence name or label).
pub(crate) fn matching(query: &str) -> Vec<(&'static str, &'static str)> {
    let q = query.to_lowercase();
    TOOLS
        .iter()
        .copied()
        .filter(|(tool, label)| {
            q.is_empty() || tool.contains(&q) || label.to_lowercase().contains(&q)
        })
        .collect()
}

/// The active `/…` in the editor: its range and query.
pub(crate) fn active(session: &EditorSession) -> Option<(SuggestionRange, String)> {
    let state = session.state();
    let s = suggestion_state(&state, KEY)?;
    s.active
        .then(|| (s.range, s.query.clone().unwrap_or_default()))
}

/// Gets a key pressed while the menu is open, with the query; whether the
/// menu took it.
pub(crate) type KeyHandler = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

/// The plugin; `on_key` decides which of the menu's keys it takes.
pub(crate) fn plugin(on_key: KeyHandler) -> Plugin {
    let mut trigger = SuggestionTrigger::default();
    trigger.char = "/".into();
    trigger.start_of_line = true;
    trigger.allowed_prefixes = None;
    let mut options = SuggestionOptions::default();
    options.key = KEY.into();
    options.trigger = trigger;
    // The handler decides which keys the menu takes; none is taken regardless.
    options.capture_keys = Vec::new();
    options.on_key_down = Some(Arc::new(move |k| on_key(&k.event.key, &k.query)));
    suggestion(options)
}

/// Replaces `range` (the `/…` typed) with an empty `tool` block and puts
/// the caret in it.
pub(crate) fn insert(session: &EditorSession, range: SuggestionRange, tool: &str) {
    let state = session.state();
    let Ok(node) = state
        .schema()
        .node_from_json(&json!({"type": "codeBlock", "attrs": {"language": tool}}))
    else {
        return;
    };
    session.run(&insert_content_at(
        range.from,
        range.to,
        Fragment::from(node),
        InsertContentOptions::default(),
    ));
    // The new block is the first code block of this tool at or after the range.
    let doc = session.state().doc().clone();
    let mut at = None;
    doc.descendants(&mut |n, p, _, _| {
        if at.is_none()
            && p + 1 >= range.from
            && n.ty().name() == "codeBlock"
            && n.attrs().get("language").and_then(Json::as_str) == Some(tool)
        {
            at = Some(p + 1);
        }
        at.is_none()
    });
    if let Some(p) = at {
        session.run(&set_text_selection(p, p));
    }
}
