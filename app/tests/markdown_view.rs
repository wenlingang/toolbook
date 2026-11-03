use std::path::PathBuf;
use std::time::Duration;

use nsh_core::{
    Size,
    element::Root,
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use toolbook_app::markdown_view::{MarkdownView, MarkdownViewProps};

fn note(body: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("toolbook-mdv-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("n.md");
    std::fs::write(&p, body).unwrap();
    p
}

fn start(path: &std::path::Path) -> Headless {
    let path = path.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<MarkdownView>(move || MarkdownViewProps {
            path: path.clone().into(),
            run_token: 0.into(),
            select_token: 0.into(),
        }))
    };
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    Headless::start(config).expect("the engine starts")
}

fn see_part(app: &mut Headless, part: &str) {
    for _ in 0..50 {
        if app.screen().texts().iter().any(|t| t.contains(part)) {
            return;
        }
        app.wait(Duration::from_millis(100)).unwrap();
    }
    panic!("never saw {part:?}; on screen: {:?}", app.screen().texts());
}

#[test]
fn markdown_view_shows_hints_not_in_text() {
    let body = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples/排查登录-401.md"),
    )
    .unwrap();
    let p = note(&body);
    let mut app = start(&p);
    see_part(&mut app, "```jwt-decode name=jwt input=$header");
    see_part(&mut app, r#"→ {"unix":1761703600"#);
    app.wait(Duration::from_millis(1200)).unwrap();
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        body,
        "viewing must not write"
    );
}

#[test]
fn broken_block_hint_is_an_error() {
    let p =
        note("---\ntoolbook: 1\n---\n```json name=a\n{oops\n```\n```pick input=$a path=x\n```\n");
    let mut app = start(&p);
    see_part(&mut app, "→ ✗");
    see_part(&mut app, "上游 [1] $a 出错");
}

#[test]
fn status_dots_show_each_blocks_state_in_the_gutter() {
    let p = note(
        "---\ntoolbook: 1\n---\n```text name=a\nhi\n```\n```json name=b\n{oops\n```\n```http name=r\nGET http://127.0.0.1:9/\n```\n",
    );
    let mut app = start(&p);
    see_part(&mut app, "→ ✗");
    let count = |app: &Headless, id: &str| app.screen().count(&format!("id:{id}")).unwrap_or(0);
    for _ in 0..30 {
        if count(&app, "md-status-ok") == 1 {
            break;
        }
        app.wait(Duration::from_millis(100)).unwrap();
    }
    assert_eq!(count(&app, "md-status-ok"), 1);
    assert_eq!(count(&app, "md-status-error"), 1);
    assert_eq!(count(&app, "md-status-waiting"), 1);
}

#[test]
fn markdown_view_shows_failed_saves_and_empty_state() {
    let p = note("---\ntoolbook: 1\n---\n```hash name=b\nx\n```\n");
    let mut app = start(&p);
    see_part(&mut app, "```hash name=b");
    std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    app.type_text("id:markdown-view", "more").unwrap();
    app.wait(Duration::from_millis(1000)).unwrap();
    see_part(&mut app, "保存失败");

    let mut empty = start(std::path::Path::new("").to_path_buf().as_ref());
    see_part(&mut empty, "没有打开的笔记");
}

#[derive(Clone, PartialEq, Props)]
struct SwitchProps {
    path: String,
}

/// The Markdown view with a button that switches the theme to Dracula.
#[component]
fn Switch(props: SwitchProps) -> impl IntoElement {
    view()
        .flex_1()
        .child(text("切换主题").on_press(|| {
            let theme = toolbook_app::themes::builtin_theme("dracula").unwrap();
            nsh_core::element::set_tokens(toolbook_app::themes::with_syntax_colors(theme).into_tokens());
        }))
        .child(toolbook_app::markdown_view::markdown_view().path(props.path.clone()))
}

#[test]
fn switching_the_theme_recolors_the_markdown_view() {
    let p = note("---\ntoolbook: 1\n---\n```text name=qq7\nx\n```\n```hash input=$qq7\n```\n");
    let path = p.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<Switch>(move || SwitchProps { path: path.clone() }))
    };
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).unwrap();
    see_part(&mut app, "input=$qq7");
    let before = app.screen().span_style("text:$qq7").unwrap().color;
    app.press("text:切换主题").unwrap();
    app.wait(Duration::from_millis(200)).unwrap();
    let after = app.screen().span_style("text:$qq7").unwrap().color;
    assert_ne!(before, after);
    let hex = |c: nsh_core::Color| {
        let c = nsh_core::Rgba::from(c);
        let to = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02x}{:02x}{:02x}", to(c.r), to(c.g), to(c.b))
    };
    assert_eq!(hex(after), "#ffb86c");
}
