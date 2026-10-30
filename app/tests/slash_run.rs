use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use nsh_core::{
    Size,
    element::Root,
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use toolbook_app::editor::note_editor;

fn note(name: &str, body: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("toolbook-slash-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[derive(Clone, PartialEq, Props)]
struct HarnessProps {
    path: String,
}

/// The editor with a button that asks it to run everything.
#[component]
fn Harness(props: HarnessProps) -> impl IntoElement {
    let token = signal(0u32);
    let select = signal(0u32);
    view()
        .flex_1()
        .child(text("运行全部").on_press(move || token.update(|t| *t += 1)))
        .child(text("全选").on_press(move || select.update(|t| *t += 1)))
        .child(
            note_editor()
                .path(props.path.clone())
                .run_token(token)
                .select_token(select),
        )
}

fn start(path: &std::path::Path) -> Headless {
    let path = path.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<Harness>(move || HarnessProps {
            path: path.clone(),
        }))
    };
    config.environment.os = Os::MacOs;
    config.environment.apple_keyboard = true;
    config.environment.idiom = Idiom::Desktop;
    Headless::start(config).expect("the engine starts")
}

fn wait_file(p: &std::path::Path, part: &str) {
    for _ in 0..40 {
        if std::fs::read_to_string(p).unwrap().contains(part) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "{} never contained {part:?}:\n{}",
        p.display(),
        std::fs::read_to_string(p).unwrap()
    );
}

#[test]
fn slash_inserts_tool_block() {
    let p = note("s.md", "---\ntoolbook: 1\n---\n说明\n");
    let mut app = start(&p);
    app.wait_for("text:说明").unwrap();
    app.press("text:说明").unwrap();
    app.press_key("end").unwrap();
    app.press_key("enter").unwrap();
    app.type_text("id:note-editor", "/jw").unwrap();
    app.wait_for("text:JWT 解码").unwrap();
    app.press_key("enter").unwrap();
    app.wait(Duration::from_millis(300)).unwrap();
    wait_file(&p, "```jwt-decode");
    assert!(!std::fs::read_to_string(&p).unwrap().contains("/jw"));
}

#[test]
fn run_all_executes_http_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let mut s = s.unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            h.fetch_add(1, Ordering::SeqCst);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}"
            );
        }
    });
    let p = note(
        "h.md",
        &format!(
            "---\ntoolbook: 1\n---\n```http name=r\nGET http://127.0.0.1:{port}/\n```\n```pick input=$r path=body.ok\n```\n"
        ),
    );
    let mut app = start(&p);
    app.wait_for("text:按 ⌘↵ 运行").unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    app.press("text:运行全部").unwrap();
    app.wait_for("text:true").unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Typing elsewhere runs the note again, reusing the response.
    app.press("text:true").ok();
    app.wait(Duration::from_millis(500)).unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn slash_menu_lets_other_keys_through() {
    let p = note("k.md", "---\ntoolbook: 1\n---\n说明\n");
    let mut app = start(&p);
    app.wait_for("text:说明").unwrap();
    app.press("text:说明").unwrap();
    app.press_key("end").unwrap();
    app.press_key("enter").unwrap();
    app.type_text("id:note-editor", "/zz").unwrap();
    app.press_key("backspace").unwrap();
    app.press_key("enter").unwrap(); // nothing matches "/z": a new line
    app.type_text("id:note-editor", "next").unwrap();
    wait_file(&p, "next");
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(!saved.contains("/zz"), "{saved}");
    assert!(!saved.contains("/znext"), "{saved}");
}

#[derive(Clone, PartialEq, Props)]
struct BothProps {
    path: String,
}

/// Both views of one note, switched by a button, sharing the run button.
#[component]
fn Both(props: BothProps) -> impl IntoElement {
    let token = signal(0u32);
    let markdown = signal(false);
    let path = props.path.clone();
    view()
        .flex_1()
        .child(text("运行全部").on_press(move || token.update(|t| *t += 1)))
        .child(text("切换视图").on_press(move || markdown.update(|m| *m = !*m)))
        .child(dynamic(move || {
            let path = path.clone();
            Some(if markdown.get() {
                toolbook_app::markdown_view::markdown_view()
                    .path(path)
                    .run_token(token)
                    .into_element()
            } else {
                note_editor().path(path).run_token(token).into_element()
            })
        }))
}

#[test]
fn run_results_survive_switching_views() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let mut s = s.unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            h.fetch_add(1, Ordering::SeqCst);
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}"
            );
        }
    });
    let p = note(
        "v.md",
        &format!(
            "---\ntoolbook: 1\n---\n```http name=r\nGET http://127.0.0.1:{port}/\n```\n```pick input=$r path=body.ok\n```\n"
        ),
    );
    let path = p.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<Both>(move || BothProps { path: path.clone() }))
    };
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:按 ⌘↵ 运行").unwrap();
    app.press("text:运行全部").unwrap();
    app.wait_for("text:true").unwrap();
    app.press("text:切换视图").unwrap();
    app.wait_for("text:→ true").unwrap();
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "the response is reused, not fetched again"
    );
    app.press("text:运行全部").unwrap();
    app.wait(Duration::from_millis(500)).unwrap();
    assert_eq!(
        hits.load(Ordering::SeqCst),
        2,
        "⌘↵ works in the Markdown view"
    );
}

#[test]
fn select_all_then_delete_clears_the_note() {
    let p = note(
        "a.md",
        "---\ntoolbook: 1\n---\n第一段\n\n```hash name=b\nx\n```\n\n最后一段\n",
    );
    let mut app = start(&p);
    app.wait_for("text:第一段").unwrap();
    app.press("text:第一段").unwrap();
    app.press("text:全选").unwrap();
    app.press_key("backspace").unwrap();
    app.wait(Duration::from_millis(1200)).unwrap();
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(
        !saved.contains("第一段") && !saved.contains("最后一段") && !saved.contains("```hash"),
        "{saved}"
    );
}

#[test]
fn select_all_marks_tool_blocks_selected() {
    let p = note(
        "sel.md",
        "---\ntoolbook: 1\n---\n第一段\n\n```hash name=b\nx\n```\n\n```base64 name=c\naGk=\n```\n\n最后一段\n",
    );
    let mut app = start(&p);
    app.wait_for("text:第一段").unwrap();
    assert_eq!(app.screen().count("label:工具块（已选中）"), Ok(0));
    app.press("text:第一段").unwrap();
    app.press("text:全选").unwrap();
    app.wait(Duration::from_millis(200)).unwrap();
    assert_eq!(app.screen().count("label:工具块（已选中）"), Ok(2));
}
