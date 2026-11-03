//! A tool block in the editor: its tool and settings above, its input (the
//! code block's own editable text) in the middle, its result below.

use nsh_base::rich_editor::model::Node;
use nsh_base::rich_editor::state::Decoration;
use nsh_base::rich_editor::view::{NodeView, NodeViewCx};
use nsh_base::theme;
use nsh_core::prelude::*;
use serde_json::Value as Json;
use toolbook_core::Value;
use toolbook_core::engine::{BlockResult, BlockStatus};
use toolbook_core::fence::parse_info;

use crate::bridge::block_is_tool;

const MONO: &str = "monospace";
/// Longest output shown in a block before it is cut.
const MAX_CHARS: usize = 4000;
const MAX_ROWS: usize = 200;

/// Renders `codeBlock` nodes: tool blocks with their results, other code as code.
pub(crate) struct ToolBlock {
    /// The note's last run, in block order.
    pub(crate) results: Signal<Vec<BlockResult>>,
    /// Whether the note says `toolbook: 1`.
    pub(crate) toolbook: bool,
    /// The note's file (where typed secrets belong).
    pub(crate) path: std::path::PathBuf,
    /// Raised when a secret is typed, to run the note again.
    pub(crate) secrets_changed: Signal<u64>,
    /// The editor's selection (`from`, `to`): a block it covers whole shows
    /// as selected, as after ⌘A.
    pub(crate) selection: Signal<(usize, usize)>,
    /// The settings line being typed in, if any: ⌘A selects its text, not
    /// the whole note.
    pub(crate) info_focus: Signal<Option<InfoFocus>>,
}

/// A block's settings line with the keyboard focus.
#[derive(Clone, Copy)]
pub(crate) struct InfoFocus {
    /// Which block's (one per element made).
    id: u64,
    input: ElementRef,
    /// Its text's length, in UTF-16 units.
    len: u32,
}

impl PartialEq for InfoFocus {
    fn eq(&self, other: &Self) -> bool {
        (self.id, self.len) == (other.id, other.len)
    }
}

impl InfoFocus {
    /// Selects the whole line.
    pub(crate) fn select_all(&self) {
        self.input.set_selection(0, self.len);
    }
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

fn language(node: &Node) -> String {
    node.attrs()
        .get("language")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_owned()
}

/// The block's place among the note's tool blocks.
fn tool_index(doc: &Node, pos: usize, toolbook: bool) -> Option<usize> {
    let mut count = 0;
    let mut found = None;
    doc.descendants(&mut |n, p, _, _| {
        if found.is_some() {
            return false;
        }
        if n.ty().name() == "codeBlock" {
            if p == pos {
                found = Some(count);
            } else if block_is_tool(&language(n), toolbook) {
                count += 1;
            }
            return false;
        }
        true
    });
    found
}

fn cut(s: &str) -> String {
    if s.chars().count() > MAX_CHARS {
        let mut c: String = s.chars().take(MAX_CHARS).collect();
        c.push_str("\n…（已截断）");
        c
    } else {
        s.to_owned()
    }
}

/// The text a result shows, and whether it is a problem.
pub(crate) fn describe(r: &BlockResult) -> (String, Tone) {
    match &r.status {
        BlockStatus::Ok(_) if r.tool == "secret" => ("已读取密钥（不显示）".into(), Tone::Muted),
        BlockStatus::Ok(Value::Text(t)) => (cut(t), Tone::Ok),
        BlockStatus::Ok(Value::Json(j)) => (
            cut(&serde_json::to_string_pretty(j).unwrap_or_default()),
            Tone::Ok,
        ),
        BlockStatus::Ok(Value::Table(t)) => {
            let mut shown = t.clone();
            let more = shown.rows.len().saturating_sub(MAX_ROWS);
            shown.rows.truncate(MAX_ROWS);
            let mut s = format!("{} 行 × {} 列\n", t.rows.len(), t.columns.len());
            s.push_str(&Value::Table(shown).render_plain());
            if more > 0 {
                s.push_str(&format!("…还有 {more} 行"));
            }
            (s, Tone::Ok)
        }
        BlockStatus::Error(m) => (format!("✗ {m}"), Tone::Error),
        BlockStatus::Incomplete(m) => (format!("… {m}"), Tone::Muted),
        BlockStatus::Upstream { block, name } => {
            (format!("上游 [{}] ${name} 出错", block + 1), Tone::Error)
        }
        BlockStatus::NotRun => ("按 ⌘↵ 运行".into(), Tone::Muted),
    }
}

/// How a result reads.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Ok,
    Error,
    Muted,
}

impl NodeView for ToolBlock {
    fn create(&self, cx: &NodeViewCx, node: &Node) -> AnyElement {
        let content = cx.take_content();
        let node_sig = cx.node();
        let selected = cx.selected();
        let info_str = language(node);
        let slot = view().px(px(10.0)).py(px(6.0)).child(content);

        if info_str == crate::bridge::RAW {
            // Markdown the editor cannot show: its text, saved as written.
            return view()
                .my(px(4.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(move || theme().border)
                .child(
                    text("原文 · 按 Markdown 原样保存")
                        .text_size(px(11.0))
                        .px(px(10.0))
                        .pt(px(4.0))
                        .text_color(move || theme().muted_foreground),
                )
                .child(slot)
                .into_element();
        }
        if !block_is_tool(&info_str, self.toolbook) {
            let plain = view().my(px(4.0)).rounded(px(6.0)).bg(move || theme().muted);
            // A tool's name in a note not marked as Toolbook's (a `/` insert,
            // say): plain code, and the reason why.
            let names_a_tool =
                toolbook_core::doc::TOOL_NAMES.contains(&parse_info(&info_str).tool.as_str());
            let plain = if !self.toolbook && names_a_tool {
                plain.child(
                    text("这篇笔记的开头没有 toolbook: 1，没写 name= 的块按普通代码处理")
                        .text_size(px(11.0))
                        .px(px(10.0))
                        .pt(px(4.0))
                        .text_color(move || theme().muted_foreground),
                )
            } else {
                plain
            };
            return plain.child(slot).into_element();
        }

        let results = self.results;
        let toolbook = self.toolbook;
        let session = cx.session.clone();
        let get_pos = cx.get_pos();
        // Its result, while it is still a tool block: a block whose info no
        // longer names a tool has none, rather than the next block's.
        let result = move || {
            let list = results.get();
            if !block_is_tool(&language(&node_sig.get()), toolbook) {
                return None;
            }
            let pos = get_pos()?;
            let index = tool_index(session.state().doc(), pos, toolbook)?;
            list.get(index).cloned()
        };
        let still_tool = move || block_is_tool(&language(&node_sig.get()), toolbook);
        let result_for_dot = result.clone();
        let result_for_color = result.clone();

        let info_value = move || language(&node_sig.get());
        let tool = parse_info(&info_str).tool;
        // An id per tool (`block-info-http`), for tests and accessibility tools.
        let info_id = if toolbook_core::doc::TOOL_NAMES.contains(&tool.as_str()) {
            format!("block-info-{tool}")
        } else {
            "block-info".to_owned()
        };
        let name_of = move || {
            parse_info(&language(&node_sig.get()))
                .name
                .map(|n| format!("${n}"))
                .unwrap_or_default()
        };
        let edit_session = cx.session.clone();
        let edit_pos = cx.get_pos();
        let info_ref = element_ref();
        let info_focus = self.info_focus;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let focus_len = move |len: u32| {
            info_focus.set(Some(InfoFocus { id, input: info_ref, len }));
        };
        let line_len = move || utf16_len(&language(&node_sig.get_untracked()));

        // Selected: picked as a whole, or its text all inside the selection
        // (⌘A in the block, or a selection over it).
        let selection = self.selection;
        let cover_pos = cx.get_pos();
        let marked = move || {
            if selected.get() {
                return true;
            }
            let (from, to) = selection.get();
            let size = node_sig.get().node_size();
            from < to && cover_pos().is_some_and(|p| from <= p + 1 && p + size - 1 <= to)
        };
        let marked_bg = marked.clone();
        let marked_label = marked.clone();
        view()
            .my(px(6.0))
            .rounded(px(8.0))
            .label(move || {
                SharedString::from(if marked_label() {
                    "工具块（已选中）"
                } else {
                    "工具块"
                })
            })
            .bg(move || {
                if marked_bg() {
                    theme().accent
                } else {
                    theme().card
                }
            })
            .border_2()
            .border_color(move || {
                if marked() {
                    theme().primary
                } else {
                    theme().border
                }
            })
            // Clip the parts to the rounded box, so no part covers a corner.
            .overflow_hidden()
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(4.0))
                    // The settings line takes its own presses: the editor
                    // would take the focus from it, and select in the
                    // block's text on a double click.
                    // A press here goes to its field (a double one selects
                    // the whole line).
                    .on_press(move |e: &PressEvent| {
                        info_ref.focus();
                        if e.clicks >= 2 {
                            info_ref.set_selection(0, line_len());
                        }
                    })
                    .child(view().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(move || {
                        let t = theme();
                        match result_for_dot().map(|r| describe(&r).1) {
                            Some(Tone::Ok) => t.color("ok").unwrap_or(t.primary),
                            Some(Tone::Error) => t.destructive,
                            _ => t.muted_foreground,
                        }
                    }))
                    .child(
                        // The info string, editable: tool, name and settings.
                        nsh_core::element::input()
                            .test_id(&info_id)
                            .element_ref(&info_ref)
                            .value(info_value)
                            .font_family(MONO)
                            .text_size(px(12.0))
                            .text_color(move || theme().primary)
                            .flex_1()
                            .on_focus(move || focus_len(line_len()))
                            .on_blur(move || {
                                if info_focus.get_untracked().is_some_and(|f| f.id == id) {
                                    info_focus.set(None);
                                }
                            })
                            .on_change(move |v: &str| {
                                if info_focus.get_untracked().is_some_and(|f| f.id == id) {
                                    focus_len(utf16_len(v));
                                }
                                let Some(pos) = edit_pos() else { return };
                                let mut tr = edit_session.state().tr();
                                if tr
                                    .set_node_attribute(pos, "language", serde_json::Value::from(v))
                                    .is_ok()
                                {
                                    let _ = edit_session.dispatch(tr);
                                }
                            }),
                    )
                    .child(
                        text(name_of)
                            .font_family(MONO)
                            .text_size(px(12.0))
                            .text_color(move || {
                                theme()
                                    .color("syntax-variable")
                                    .unwrap_or(theme().accent_foreground)
                            }),
                    ),
            )
            .child(if tool == "secret" {
                // A secret is typed here, masked, and kept in memory only.
                let path = self.path.clone();
                let name = parse_info(&info_str).name.unwrap_or_default();
                let changed = self.secrets_changed;
                let shown = crate::secrets::get(&path, &name).unwrap_or_default();
                view()
                    .px(px(10.0))
                    .py(px(6.0))
                    .child(
                        nsh_core::element::input()
                            .test_id("block-secret")
                            .masked(true)
                            .value(shown)
                            .placeholder("输入密钥（只保存在内存里，不写进文件）")
                            .font_family(MONO)
                            .text_size(px(12.0))
                            .on_change(move |v: &str| {
                                crate::secrets::set(&path, &name, v);
                                changed.update(|n| *n += 1);
                            }),
                    )
                    .into_element()
            } else {
                slot.into_element()
            })
            .child(
                view()
                    .px(px(10.0))
                    .py(px(6.0))
                    // Inside the 2pt border of an 8pt box.
                    .rounded_b(px(6.0))
                    .bg(move || theme().muted)
                    .child(
                        text(move || match result() {
                            Some(r) => {
                                let (mut s, _) = describe(&r);
                                for n in &r.notes {
                                    s.push_str(&format!("\n· {n}"));
                                }
                                s
                            }
                            None if !still_tool() => "不是已知工具，这个块按普通代码处理".into(),
                            None => "…".into(),
                        })
                        .font_family(MONO)
                        .text_size(px(12.0))
                        .text_color({
                            move || {
                                let t = theme();
                                match result_for_color().map(|r| describe(&r).1) {
                                    Some(Tone::Error) => t.destructive,
                                    Some(Tone::Muted) | None => t.muted_foreground,
                                    Some(Tone::Ok) => t.foreground,
                                }
                            }
                        }),
                    ),
            )
            .into_element()
    }

    fn editable_content_slot(&self) -> bool {
        true
    }

    fn update(&self, node: &Node, _decorations: &[Decoration]) -> bool {
        // A block that turns into a tool (or stops being one) is made again.
        let _ = node;
        true
    }
}
