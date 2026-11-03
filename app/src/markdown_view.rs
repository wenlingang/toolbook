//! The Markdown view (design C): the note's source in a code editor, its
//! own colors from the theme, and each tool block's result as a hint after
//! the block's closing fence. Hints are drawn over the editor, never in
//! its text, so copying or saving never takes them.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use nsh_base::{
    Editor, EditorApi, EditorPosition, EditorRange, EditorScrollBeyond, EditorSelection, editor,
    theme,
};
use nsh_core::PointerEvents;
use nsh_core::element::component_ref;
use nsh_core::prelude::*;
use toolbook_core::doc::{Document, Segment};
use toolbook_core::engine::{BlockResult, BlockStatus};

use crate::highlight::decorations;

/// The editor's type and rows. Menlo is on every Mac; nsh uses the first
/// named family only, so a missing one would fall back to the system's
/// proportional font.
const LINE_HEIGHT: f32 = 24.0;
const FONT_SIZE: f32 = 14.0;
const MONO: &str = "Menlo";
/// The space between a closing fence and its hint.
const HINT_GAP: f32 = 20.0;
/// How long after the last scroll the hints may be measured again.
const SETTLE: Duration = Duration::from_millis(150);
const SAVE_DELAY: Duration = Duration::from_millis(600);
const RUN_DELAY: Duration = Duration::from_millis(150);

/// The props of [`MarkdownView`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MarkdownViewProps {
    /// The note's file.
    pub path: Prop<SharedString>,
    /// Raised by one to run every block, `http` included (⌘↵).
    #[prop(default = 0)]
    pub run_token: Prop<u32>,
    /// Raised by one to select the whole text (⌘A).
    #[prop(default = 0)]
    pub select_token: Prop<u32>,
}

/// The line of each tool block's closing fence, in block order.
pub fn closing_lines(doc: &Document) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut line = 0u32;
    for seg in &doc.segments {
        match seg {
            Segment::Raw(t) => line += t.matches('\n').count() as u32,
            Segment::Block(b) => {
                let lines: Vec<&str> = b.raw.split('\n').collect();
                // The raw text ends with the closing fence's line break (or not, at the end).
                let close = if b.raw.ends_with('\n') {
                    lines.len() - 2
                } else {
                    lines.len() - 1
                };
                out.push((
                    line + close as u32,
                    lines[close].trim_end_matches('\r').to_owned(),
                ));
                line += b.raw.matches('\n').count() as u32;
            }
        }
    }
    out
}

/// A block's hint: what it gave, or why not.
pub fn hint(r: &BlockResult) -> (String, bool) {
    match &r.status {
        BlockStatus::Ok(_) if r.tool == "secret" => ("→ 已读取密钥".into(), false),
        BlockStatus::Ok(v) => (format!("→ {}", v.summary()), false),
        BlockStatus::Error(m) => (format!("→ ✗ {m}"), true),
        BlockStatus::Upstream { block, name } => {
            (format!("→ 上游 [{}] ${name} 出错", block + 1), true)
        }
        BlockStatus::Incomplete(m) => (format!("→ … {m}"), false),
        BlockStatus::NotRun => ("→ 按 ⌘↵ 运行".into(), false),
    }
}

/// The note's source, editable, with results beside its tool blocks.
#[component]
pub fn MarkdownView(props: MarkdownViewProps) -> impl IntoElement {
    view()
        .flex_1()
        .test_id("markdown-view")
        .child(dynamic(move || {
            let path = PathBuf::from(props.path.get().to_string());
            untracked(move || open(path, props.run_token, props.select_token))
        }))
}

fn open(path: PathBuf, run_token: Prop<u32>, select_token: Prop<u32>) -> AnyElement {
    if path.as_os_str().is_empty() {
        return text("没有打开的笔记")
            .m(px(24.0))
            .text_color(move || theme().muted_foreground)
            .into_element();
    }
    let save_error = signal(None::<String>);
    let initial = std::fs::read_to_string(&path).unwrap_or_default();
    let source = signal(initial.clone());
    let results = signal(Vec::<BlockResult>::new());
    let runner = crate::runner::Runner::new(&path, move |report| results.set(report.blocks));
    let rerun = {
        let runner = runner.clone();
        move |text: String, delay: Duration, run_manual: bool| runner.request(text, delay, run_manual)
    };
    rerun(initial, Duration::ZERO, false);
    {
        let rerun = rerun.clone();
        let start = run_token.get_untracked();
        effect(move || {
            let t = run_token.get();
            if t != start {
                untracked(|| rerun(source.get_untracked(), Duration::ZERO, true));
            }
        });
    }

    // Edits are saved after a pause, and when the view goes.
    let pending: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let save_gen = Rc::new(Cell::new(0u64));
    {
        let pending = pending.clone();
        let path = path.clone();
        cleanup(move || {
            // Renamed, moved or deleted meanwhile: not written back there.
            if let Some(text) = pending.borrow_mut().take()
                && path.exists()
            {
                let _ = std::fs::write(&path, text);
            }
        });
    }

    // The editor grows with its text inside a scroll view of ours, and the
    // hints sit beside it in that scroll view's content, so a scroll moves
    // both at once. Where each hint sits is read from the editor's geometry
    // only after the text or the results change, and not while the view
    // scrolls: the editor measures where its rows are against the window,
    // which a scroll in progress throws off, and hints placed by it then
    // jump and are rebuilt (they flicker). Lines wrap, so a new width
    // moves them too.
    let editor_ref = component_ref::<Editor>();
    let outer = element_ref();
    let shown = Rc::new(Cell::new((0.0f32, 0.0f32)));
    let last_scroll = Rc::new(Cell::new(None::<Instant>));
    let placed = signal(Vec::<(f32, f32, f32)>::new());
    let stale = Rc::new(Cell::new(true));
    let resized = stale.clone();
    {
        let stale = stale.clone();
        effect(move || {
            source.with(|_| ());
            results.with(|_| ());
            stale.set(true);
        });
    }
    let frames = {
        let last_scroll = last_scroll.clone();
        spawn(async move {
            loop {
                next_frame().await;
                let Some(api) = editor_ref.api::<EditorApi>() else {
                    continue;
                };
                let o = api.scroll_offset();
                let scrolling = last_scroll.get().is_some_and(|t| t.elapsed() < SETTLE);
                if !stale.get() || scrolling {
                    continue;
                }
                stale.set(false);
                let at: Vec<(f32, f32, f32)> = source
                    .with_untracked(|s| closing_lines(&Document::parse(s)))
                    .iter()
                    .map(|(line, text)| {
                        let at = EditorPosition::new(*line, text.encode_utf16().count() as u32);
                        api.range_bounds(EditorRange::new(at, at))
                            .into_iter()
                            .next()
                            .map_or((0.0, 0.0, 0.0), |b| {
                                let r = b.bounds;
                                (r.origin.x.0 + o.x.0, r.origin.y.0 + o.y.0, r.size.height.0)
                            })
                    })
                    .collect();
                // Laid out a frame later than the text changed: measured again then.
                if at.iter().any(|p| p.2 <= 0.0) {
                    stale.set(true);
                }
                if at != placed.get_untracked() {
                    placed.set(at);
                }
            }
        })
    };
    cleanup(move || drop(frames));
    // The editor cannot scroll its caret into view any more (it shows all
    // its rows): the scroll view follows the caret instead.
    let follow_caret = {
        let shown = shown.clone();
        move || {
            let shown = shown.clone();
            let outer = outer.clone();
            request_animation_frame(move |_| {
                let Some(caret) = editor_ref
                    .api::<EditorApi>()
                    .and_then(|api| api.caret_bounds())
                else {
                    return;
                };
                let (top, height) = shown.get();
                let (y, row) = (caret.bounds.origin.y.0, caret.bounds.size.height.0);
                let to = if y < top {
                    y
                } else if y + row > top + height {
                    y + row - height
                } else {
                    return;
                };
                outer.scroll_to(Point::new(px(0.0), px(to.max(0.0))), false);
            });
        }
    };
    {
        let start = select_token.get_untracked();
        effect(move || {
            if select_token.get() != start
                && let Some(api) = untracked(|| editor_ref.api::<EditorApi>())
            {
                api.run(nsh_base::EditorCommand::SelectAll);
                api.focus();
            }
        });
    }
    let hints = move || {
        let list = results.get();
        placed
            .get()
            .into_iter()
            .zip(list)
            .filter(|((_, _, row), _)| *row > 0.0)
            .flat_map(|((x, y, row), r)| {
                let (label, bad) = hint(&r);
                [status_dot(&r, y, row), text_hint(label, bad, x + HINT_GAP, y, row)]
            })
            .collect::<Vec<_>>()
    };

    view()
        .flex_1()
        .child(dynamic(move || {
            save_error.get().map(|m| {
                text(m).px(px(10.0)).py(px(6.0)).text_size(px(12.0)).text_color(move || theme().destructive).into_element()
            })
        }))
        .child(
            scroll()
                .element_ref(&outer)
                .scrollbar(false)
                .scrollbar(false)
                .flex_1()
                .on_scroll(move |e: &ScrollEvent| {
                    shown.set((e.offset.y.0, e.size.height.0));
                    last_scroll.set(Some(Instant::now()));
                })
                .child(
                    view()
                        .relative()
                        .on_layout(move |_: &Bounds<Points>| resized.set(true))
                        .child(
                    editor()
                        .component_ref(&editor_ref)
                        .value(move || Some(SharedString::from(source.get())))
                        .language("markdown")
                        .highlighting(false)
                        .decorations(move || decorations(&source.get()))
                        .font_family(MONO)
                        .font_size(FONT_SIZE)
                        .line_height(LINE_HEIGHT)
                        .soft_wrap(true)
                        .min_rows(1)
                        .scroll_beyond_last_line(EditorScrollBeyond::None)
                        .w_full()
                        .on_selection_change(move |_: &Vec<EditorSelection>| follow_caret())
                        .on_change(move |v: &SharedString| {
                            let text = v.to_string();
                            source.set(text.clone());
                            rerun(text.clone(), RUN_DELAY, false);
                            *pending.borrow_mut() = Some(text.clone());
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
                                let Some(text) = pending.borrow_mut().take() else { return };
                                if !path.exists() {
                                    save_error.set(Some("保存失败：笔记文件已被移动或删除，最近的修改没有保存".into()));
                                    return;
                                }
                                match std::fs::write(&path, &text) {
                                    Ok(()) => save_error.set(None),
                                    Err(e) => {
                                        save_error.set(Some(format!("保存失败：{e}（{}）", path.display())));
                                        *pending.borrow_mut() = Some(text);
                                    }
                                }
                            });
                        }),
                )
                        .child(dynamic(move || {
                            Some(
                                view()
                                    .absolute()
                                    .inset(px(0.0))
                                    .overflow_hidden()
                                    .pointer_events(PointerEvents::None)
                                    .children(hints()),
                            )
                        })),
                ),
        )
        .into_element()
}

/// A block's state at a glance, in the gutter beside its closing fence:
/// the document view's colors (done, broken, waiting).
fn status_dot(r: &BlockResult, y: f32, row: f32) -> AnyElement {
    #[derive(Clone, Copy)]
    enum State {
        Ok,
        Error,
        Waiting,
    }
    let state = match &r.status {
        BlockStatus::Ok(_) => State::Ok,
        BlockStatus::Error(_) | BlockStatus::Upstream { .. } => State::Error,
        BlockStatus::Incomplete(_) | BlockStatus::NotRun => State::Waiting,
    };
    const SIZE: f32 = 6.0;
    view()
        .test_id(match state {
            State::Ok => "md-status-ok",
            State::Error => "md-status-error",
            State::Waiting => "md-status-waiting",
        })
        .absolute()
        .top(px(y + (row - SIZE) / 2.0))
        .left(px(4.0))
        .w(px(SIZE))
        .h(px(SIZE))
        .rounded(px(SIZE / 2.0))
        .bg(move || {
            let t = theme();
            match state {
                State::Ok => t.color("ok").unwrap_or(t.primary),
                State::Error => t.destructive,
                State::Waiting => t.muted_foreground,
            }
        })
        .into_element()
}

fn text_hint(label: String, bad: bool, x: f32, y: f32, row: f32) -> AnyElement {
    text(label)
        .absolute()
        .top(px(y + (row - FONT_SIZE - 4.0).max(0.0) / 2.0))
        .left(px(x))
        .font_family(MONO)
        .text_size(px(FONT_SIZE - 1.0))
        .italic()
        .text_color(move || {
            if bad {
                theme().destructive
            } else {
                theme().muted_foreground
            }
        })
        .into_element()
}
