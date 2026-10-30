//! The note editor: one note's document, run again as it changes and saved
//! back to its file.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use nsh_base::rich_editor::extensions::{markdown::markdown, starter};
use nsh_base::rich_editor::state::{EditorState, EditorStateConfig};
use nsh_base::rich_editor::view::{EditorSession, NodeViews, RichEditorClasses, rich_editor};
use nsh_base::theme;
use nsh_core::prelude::*;
use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockResult, BlockStatus, Memo, RunOptions, run};

use crate::blocks::ToolBlock;
use crate::bridge::{schema, to_editor_json, to_markdown};
use crate::slash;

/// Waits this long after a change before running the note again.
const RUN_DELAY: Duration = Duration::from_millis(150);
/// Waits this long after a change before saving.
const SAVE_DELAY: Duration = Duration::from_millis(800);

/// The props of [`NoteEditor`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NoteEditorProps {
    /// The note's file.
    pub path: Prop<SharedString>,
    /// Raised by one to run every block, `http` included (⌘↵).
    #[prop(default = 0)]
    pub run_token: Prop<u32>,
    /// Raised by one to select the whole note (⌘A).
    #[prop(default = 0)]
    pub select_token: Prop<u32>,
}

/// The events of a [`NoteEditor`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum NoteEditorEvent {
    /// The document changed; its Markdown.
    #[args(markdown)]
    Changed(String),
    /// The note was written to its file.
    #[args(path)]
    Saved(String),
    /// A line about the last run: "4 个块 · 1 个出错".
    #[args(text)]
    Status(String),
}

/// How the document reads: headings, paragraphs, lists, quotes and code.
fn classes() -> RichEditorClasses {
    let nodes: std::collections::BTreeMap<String, String> = [
        ("paragraph", "py-1 text-[15px] leading-6 text-foreground"),
        (
            "heading[level=1]",
            "pt-3 pb-2 text-2xl font-bold text-foreground",
        ),
        (
            "heading[level=2]",
            "pt-3 pb-1 text-xl font-semibold text-foreground",
        ),
        (
            "heading[level=3]",
            "pt-2 pb-1 text-lg font-semibold text-foreground",
        ),
        ("bulletList", "pl-6"),
        ("orderedList", "pl-6"),
        (
            "blockquote",
            "pl-3 border-l-2 border-border text-muted-foreground",
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let marks: std::collections::BTreeMap<String, String> = [
        ("bold", "font-bold"),
        ("italic", "italic"),
        ("code", "font-mono bg-muted text-syntax-variable"),
        ("link", "text-primary underline"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let mut c = RichEditorClasses::default();
    c.content = Some("pb-24".into());
    c.list_marker = Some("text-muted-foreground".into());
    c.nodes = Some(nodes);
    c.marks = Some(marks);
    c
}

/// A summary of a run for the window's status line.
pub fn status_line(blocks: &[BlockResult]) -> String {
    let bad = blocks
        .iter()
        .filter(|b| {
            matches!(
                b.status,
                BlockStatus::Error(_) | BlockStatus::Upstream { .. }
            )
        })
        .count();
    if bad == 0 {
        format!("{} 个块", blocks.len())
    } else {
        format!("{} 个块 · {bad} 个出错", blocks.len())
    }
}

/// Edits the note at `path`: its tool blocks show their results, and
/// changes are saved back to the file.
#[component]
pub fn NoteEditor(props: NoteEditorProps, cx: Scope<NoteEditorEvent>) -> impl IntoElement {
    let results = signal(Vec::<BlockResult>::new());
    view()
        .flex_1()
        .test_id("note-editor")
        .child(dynamic(move || {
            let path = PathBuf::from(props.path.get().to_string());
            untracked(move || open(path, results, props.run_token, props.select_token, cx))
        }))
}

/// Runs the note again: after `delay`, in the background, the latest
/// request only.
#[derive(Clone)]
struct Runner {
    path: PathBuf,
    generation: Rc<Cell<u64>>,
    memo: Memo,
    base_dir: PathBuf,
    results: Signal<Vec<BlockResult>>,
    cx: Scope<NoteEditorEvent>,
}

impl Runner {
    fn request(&self, markdown: String, delay: Duration, run_manual: bool) {
        let my = self.generation.get() + 1;
        self.generation.set(my);
        let this = self.clone();
        spawn(async move {
            if !delay.is_zero() {
                timer(delay).await;
            }
            if this.generation.get() != my {
                return;
            }
            let memo = this.memo.clone();
            let base_dir = this.base_dir.clone();
            let secrets = crate::secrets::overrides(&this.path);
            let job = background(async move {
                let mut opts = RunOptions::from_process(run_manual);
                opts.memo = Some(memo);
                opts.overrides = secrets;
                run(&Document::parse(&markdown), &opts, &base_dir)
            });
            if let Ok(report) = job.await
                && this.generation.get() == my
            {
                this.cx
                    .emit(NoteEditorEvent::Status(status_line(&report.blocks)));
                this.results.set(report.blocks);
            }
        });
    }
}

/// What shows while no note is open: nothing to type into, so nothing is lost.
fn empty_state() -> AnyElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .child(
            text("没有打开的笔记")
                .text_size(px(15.0))
                .text_color(move || theme().foreground),
        )
        .child(
            text("在左侧选择一本，或按 ⌘N 新建")
                .text_size(px(13.0))
                .text_color(move || theme().muted_foreground),
        )
        .into_element()
}

fn open(
    path: PathBuf,
    results: Signal<Vec<BlockResult>>,
    run_token: Prop<u32>,
    select_token: Prop<u32>,
    cx: Scope<NoteEditorEvent>,
) -> AnyElement {
    if path.as_os_str().is_empty() {
        return empty_state();
    }
    let save_error = signal(None::<String>);
    let secrets_changed = signal(0u64);
    let selection = signal((0usize, 0usize));
    let src = std::fs::read_to_string(&path).unwrap_or_default();
    let doc = Document::parse(&src);
    let schema = schema();
    let node = schema
        .node_from_json(&to_editor_json(&doc, &schema))
        .unwrap_or_else(|_| {
            schema
                .node_from_json(
                    &serde_json::json!({"type": "doc", "content": [{"type": "paragraph"}]}),
                )
                .expect("an empty doc")
        });

    // The `/` menu's state: open with a query, and the highlighted item.
    let menu = signal(None::<(usize, String)>);
    let picked = signal(0usize);
    // A key the menu took, with a counter so the same key twice is two presses.
    let menu_key = signal(None::<(u64, String)>);
    // The menu takes ↑ ↓, and Enter while something matches; every other
    // key (Backspace, a letter, Enter with no match) goes to the editor.
    let on_key: slash::KeyHandler = Arc::new(move |key: &str, query: &str| {
        let take = match key {
            "ArrowUp" | "ArrowDown" => true,
            "Enter" => !slash::matching(query).is_empty(),
            _ => false,
        };
        if take {
            let n = menu_key.get_untracked().map(|(n, _)| n + 1).unwrap_or(0);
            menu_key.set(Some((n, key.to_owned())));
        }
        take
    });

    // The `/` menu first: its keys come before the keymaps' (Enter, arrows).
    let mut plugins = vec![slash::plugin(on_key)];
    plugins.extend(markdown(Default::default()));
    plugins.extend(starter::plugins(&schema));
    let state = EditorState::create(EditorStateConfig::new(schema).doc(node).plugins(plugins))
        .expect("the editor state is valid");
    let session = EditorSession::new(
        state,
        NodeViews::new().with(
            "codeBlock",
            Arc::new(ToolBlock {
                results,
                toolbook: doc.is_toolbook,
                path: path.clone(),
                secrets_changed,
                selection,
            }),
        ),
    );

    let runner = Runner {
        path: path.clone(),
        generation: Rc::new(Cell::new(0)),
        memo: crate::memos::memo_for(&path),
        base_dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
        results,
        cx,
    };
    runner.request(src, Duration::ZERO, false);

    let front_matter = doc.front_matter.clone();
    let markdown_now = {
        let session = session.clone();
        move || to_markdown(front_matter.as_deref(), session.state().doc())
    };

    // ⌘A: the whole note, ready to copy or delete.
    {
        let session = session.clone();
        let start = select_token.get_untracked();
        effect(move || {
            if select_token.get() != start {
                untracked(|| {
                    session.run(&nsh_base::rich_editor::commands::command(
                        nsh_base::rich_editor::commands::base::select_all,
                    ));
                    session.focus();
                });
            }
        });
    }

    // A secret typed in a block: run again with it.
    {
        let runner = runner.clone();
        let markdown_now = markdown_now.clone();
        effect(move || {
            if secrets_changed.get() > 0 {
                untracked(|| runner.request(markdown_now(), RUN_DELAY, false));
            }
        });
    }

    // ⌘↵: run everything once, keeping manual results for later runs.
    {
        let runner = runner.clone();
        let markdown_now = markdown_now.clone();
        let start = run_token.get_untracked();
        effect(move || {
            let t = run_token.get();
            if t != start {
                untracked(|| runner.request(markdown_now(), Duration::ZERO, true));
            }
        });
    }

    // Keys the `/` menu captured: move the highlight, or insert.
    let handle_key = {
        let session = session.clone();
        move |key: String| {
            let Some((range, query)) = slash::active(&session) else {
                return;
            };
            let items = slash::matching(&query);
            match key.as_str() {
                "ArrowDown" => picked.update(|p| *p = (*p + 1).min(items.len().saturating_sub(1))),
                "ArrowUp" => picked.update(|p| *p = p.saturating_sub(1)),
                "Enter" => {
                    if let Some((tool, _)) = items.get(picked.get_untracked()) {
                        slash::insert(&session, range, tool);
                    }
                }
                _ => {}
            }
        }
    };
    // Handled after the editor's own transaction is done.
    effect(move || {
        if let Some((_, key)) = menu_key.get() {
            let handle_key = handle_key.clone();
            untracked(|| {
                set_timeout(Duration::ZERO, move || handle_key(key));
            });
        }
    });
    let refresh_menu = {
        let session = session.clone();
        move || {
            let now = slash::active(&session).map(|(r, q)| (r.from, q));
            if now.as_ref().map(|(_, q)| q) != menu.get_untracked().as_ref().map(|(_, q)| q) {
                picked.set(0);
            }
            menu.set(now);
        }
    };

    let save_gen = Rc::new(Cell::new(0u64));
    // An edit not saved yet is saved when the editor goes (another note,
    // the Markdown view, the window closing).
    let pending: Rc<std::cell::RefCell<Option<String>>> = Rc::new(std::cell::RefCell::new(None));
    {
        let pending = pending.clone();
        let path = path.clone();
        cleanup(move || {
            // Renamed, moved or deleted meanwhile: not written back there.
            if let Some(md) = pending.borrow_mut().take()
                && path.exists()
            {
                let _ = std::fs::write(&path, md);
            }
        });
    }
    let editor = rich_editor()
        .editor(session.clone())
        .classes(classes())
        .size_full()
        .on_change({
            let refresh_menu = refresh_menu.clone();
            move |_: &_| {
                refresh_menu();
                let md = markdown_now();
                runner.request(md.clone(), RUN_DELAY, false);
                cx.emit(NoteEditorEvent::Changed(md.clone()));
                *pending.borrow_mut() = Some(md);
                let my = save_gen.get() + 1;
                save_gen.set(my);
                let save_gen = save_gen.clone();
                let pending = pending.clone();
                let path = path.clone();
                spawn(async move {
                    timer(SAVE_DELAY).await;
                    if save_gen.get() != my {
                        return;
                    }
                    let Some(md) = pending.borrow_mut().take() else {
                        return;
                    };
                    if !path.exists() {
                        let msg = "保存失败：笔记文件已被移动或删除，最近的修改没有保存".to_owned();
                        cx.emit(NoteEditorEvent::Status(msg.clone()));
                        save_error.set(Some(msg));
                        return;
                    }
                    match std::fs::write(&path, &md) {
                        Ok(()) => {
                            save_error.set(None);
                            cx.emit(NoteEditorEvent::Saved(path.to_string_lossy().into_owned()));
                        }
                        Err(e) => {
                            let msg = format!("保存失败：{e}（{}）", path.display());
                            cx.emit(NoteEditorEvent::Status(msg.clone()));
                            save_error.set(Some(msg));
                            // Kept, so leaving the note tries once more.
                            *pending.borrow_mut() = Some(md);
                        }
                    }
                });
            }
        })
        .on_selection_change({
            let session = session.clone();
            move |_: &_| {
                refresh_menu();
                let state = session.state();
                let sel = state.selection();
                selection.set((sel.from(), sel.to()));
            }
        });

    let menu_session = session.clone();
    view()
        .size_full()
        .child(dynamic(move || {
            save_error.get().map(|m| {
                text(m)
                    .px(px(10.0))
                    .py(px(6.0))
                    .text_size(px(12.0))
                    .text_color(move || theme().destructive)
                    .into_element()
            })
        }))
        .child(editor)
        .child(dynamic(move || {
            let (from, query) = menu.get()?;
            let at = menu_session.coords_at_pos(from);
            let items = slash::matching(&query);
            let session = menu_session.clone();
            Some(
                view()
                    .absolute()
                    .top(px(at
                        .map(|b| (b.origin.y + b.size.height).0 + 4.0)
                        .unwrap_or(40.0)))
                    .left(px(at.map(|b| b.origin.x.0).unwrap_or(16.0)))
                    .w(px(220.0))
                    .p(px(4.0))
                    .rounded(px(8.0))
                    .bg(move || theme().popover)
                    .border_1()
                    .border_color(move || theme().border)
                    .children(
                        items
                            .into_iter()
                            .enumerate()
                            .map(move |(i, (tool, label))| {
                                let session = session.clone();
                                view()
                                    .flex_row()
                                    .gap(px(8.0))
                                    .px(px(8.0))
                                    .py(px(4.0))
                                    .rounded(px(4.0))
                                    .bg(move || {
                                        if picked.get() == i {
                                            theme().accent
                                        } else {
                                            theme().popover
                                        }
                                    })
                                    .on_press(move || {
                                        if let Some((range, _)) = slash::active(&session) {
                                            slash::insert(&session, range, tool);
                                        }
                                    })
                                    .child(text(label).text_size(px(13.0)))
                                    .child(
                                        text(tool)
                                            .font_family("monospace")
                                            .text_size(px(11.0))
                                            .text_color(move || theme().muted_foreground),
                                    )
                                    .into_element()
                            }),
                    )
                    .into_element(),
            )
        }))
        .into_element()
}
