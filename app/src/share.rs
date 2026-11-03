//! Showing a note to others: the preview (the shared page itself, in a web
//! view) and the two ways out of the app, an HTML file and Markdown with
//! the results on the clipboard.

use std::path::{Path, PathBuf};

use nsh_core::element::{pick_save_path, reveal_path, web_view};
use nsh_core::file_dialog::{FileFilter, SaveDialog};
use nsh_core::prelude::*;
use toolbook_core::doc::Document;
use toolbook_core::engine::{RunOptions, RunReport, run};
use toolbook_core::share::{markdown_with_results, to_html};

/// Runs the note at `path` as the app does: its kept results, its typed secrets.
fn run_note(path: &Path) -> Result<(Document, RunReport), String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取 {} 失败：{e}", path.display()))?;
    let doc = Document::parse(&text);
    let mut opts = RunOptions::from_process(false);
    opts.memo = Some(crate::memos::memo_for(path));
    opts.overrides = crate::secrets::overrides(path);
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let report = run(&doc, &opts, &base);
    Ok((doc, report))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The shared page of the note at `path`.
pub fn note_html(path: &Path) -> Result<String, String> {
    let (doc, report) = run_note(path)?;
    Ok(to_html(&doc, &report, &stem(path)))
}

/// The note at `path` as Markdown with its results.
pub fn note_markdown(path: &Path) -> Result<String, String> {
    let (doc, report) = run_note(path)?;
    Ok(markdown_with_results(&doc, &report))
}

/// Keeps a typed secret, as a secret block's input does (tests).
#[doc(hidden)]
pub fn remember_secret_for_test(path: &Path, name: &str, value: &str) {
    crate::secrets::set(path, name, value);
}

/// The shared page `html` in `theme`'s colors (the preview in the app):
/// its CSS variables set again after its own, for the light appearance and
/// the dark one, as the app picks its colors.
pub fn with_theme(html: &str, theme: &nsh_base::Theme) -> String {
    use crate::themes::{SyntaxKind, hex, syntax};
    let vars = |dark: bool| {
        let c = |color| hex(color, dark);
        let k = |kind| hex(syntax(theme, kind), dark);
        format!(
            ":root{{--bg:{};--fg:{};--muted:{};--rule:{};--accent:{};--rail:{};--in:{};--panel:{};--panel-fg:{};--k:{};--s:{};--n:{};--l:{};--error:{}}}",
            c(theme.background),
            c(theme.foreground),
            c(theme.muted_foreground),
            c(theme.border),
            c(theme.primary),
            c(theme.ring),
            c(theme.muted),
            c(theme.card),
            c(theme.card_foreground),
            k(SyntaxKind::Keyword),
            k(SyntaxKind::String),
            k(SyntaxKind::Number),
            k(SyntaxKind::Variable),
            c(theme.destructive),
        )
    };
    let style = format!(
        "<style>{}@media (prefers-color-scheme:dark){{{}}}</style></head>",
        vars(false),
        vars(true)
    );
    html.replacen("</head>", &style, 1)
}

/// Hides the web view's scroll bars (the exported page keeps them).
const HIDE_SCROLLBARS: &str = "var s=document.createElement('style');s.textContent='::-webkit-scrollbar{display:none}html{scrollbar-width:none}';document.head.appendChild(s);";

/// The props of [`PreviewView`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PreviewViewProps {
    /// The note's file.
    pub path: Prop<SharedString>,
    /// Raised by one to show the page again (after ⌘↵, after an export).
    #[prop(default = 0)]
    pub refresh_token: Prop<u32>,
}

/// The note as others will see it: the shared page itself.
#[component]
pub fn PreviewView(props: PreviewViewProps) -> impl IntoElement {
    let html = move || {
        props.refresh_token.get();
        let path = props.path.get().to_string();
        if path.is_empty() {
            return String::from(
                "<!doctype html><meta charset=utf-8><p style=\"font:15px -apple-system;color:#888;padding:24px\">没有打开的笔记</p>",
            );
        }
        // Reading the theme here shows the page again when it changes.
        let theme = nsh_base::theme();
        match note_html(Path::new(&path)) {
            Ok(h) => with_theme(&h, &theme),
            Err(e) => format!(
                "<!doctype html><meta charset=utf-8><p style=\"font:15px -apple-system;color:#d92d20;padding:24px\">{e}</p>"
            ),
        }
    };
    view()
        .flex_1()
        .test_id("preview")
        .child(
            web_view("about:blank")
                .html(html, None)
                // The preview scrolls without a bar, as the rest of the app.
                .injected_java_script(String::from(HIDE_SCROLLBARS))
                .flex_1(),
        )
}

/// Sharing a note.
#[module]
pub struct Share;

#[module]
impl Share {
    /// Asks where to save the note's shared page and writes it there; the
    /// path, or nothing when the user cancelled.
    pub async fn export_html(path: String) -> Result<Option<String>, String> {
        let note = PathBuf::from(&path);
        let html = note_html(&note)?;
        let dialog = SaveDialog::new()
            .file_name(format!("{}.html", stem(&note)))
            .filter(FileFilter::new("网页", ["html"]))
            .title("导出分享页")
            .button_label("导出");
        let Some(target) = pick_save_path(dialog).await? else {
            return Ok(None);
        };
        std::fs::write(&target, html)
            .map_err(|e| format!("写入 {} 失败：{e}", target.display()))?;
        let _ = reveal_path(target.clone()).await;
        Ok(Some(target.to_string_lossy().into_owned()))
    }

    /// Puts the note, with each block's result after it, on the clipboard.
    pub fn copy_markdown(path: String, cx: &mut ModuleCx) -> Result<(), String> {
        let md = note_markdown(Path::new(&path))?;
        cx.set_clipboard(md);
        Ok(())
    }
}
