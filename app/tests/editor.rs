use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use nsh_core::{
    Size,
    element::Root,
    platform::{Idiom, Os},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::Headless;
use toolbook_app::editor::{NoteEditor, NoteEditorProps};

fn note(name: &str, body: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("toolbook-app-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

fn start(path: &std::path::Path) -> Headless {
    let path = path.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<NoteEditor>(move || NoteEditorProps {
            path: path.clone().into(),
            run_token: 0.into(),
            select_token: 0.into(),
        }))
    };
    config.environment.os = Os::MacOs;
    config.environment.apple_keyboard = true;
    config.environment.idiom = Idiom::Desktop;
    Headless::start(config).expect("the engine starts")
}

/// Waits until some text on screen contains `part`.
fn see_part(app: &mut Headless, part: &str) {
    for _ in 0..50 {
        if app.screen().texts().iter().any(|t| t.contains(part)) {
            return;
        }
        app.wait(Duration::from_millis(100)).unwrap();
    }
    panic!("never saw {part:?}; on screen: {:?}", app.screen().texts());
}

/// Types `s` into the focused document, key by key.
fn keys(app: &mut Headless, s: &str) {
    for c in s.chars() {
        app.press_key(&c.to_string()).unwrap();
    }
}

fn example(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../examples")
            .join(name),
    )
    .unwrap()
}

#[test]
fn renders_example_outputs() {
    let p = note("a.md", &example("排查登录-401.md"));
    let mut app = start(&p);
    see_part(&mut app, r#""unix": 1761703600,"#);
}

#[test]
fn upstream_error_is_shown() {
    let p = note(
        "b.md",
        "---\ntoolbook: 1\n---\n```text name=a\n```\n```base64 name=b input=$a\n```\n",
    );
    let mut app = start(&p);
    app.wait_for("text:上游 [1] $a 出错").unwrap();
}

#[test]
fn opening_without_edit_does_not_write() {
    let p = note("c.md", &example("csv-to-sql.md"));
    let before = std::fs::metadata(&p).unwrap().modified().unwrap();
    let mut app = start(&p);
    see_part(&mut app, "INSERT INTO campaign_whitelist");
    app.wait(Duration::from_millis(1500)).unwrap();
    drop(app);
    let after = std::fs::metadata(&p).unwrap().modified().unwrap();
    assert_eq!(before, after);
    let _ = SystemTime::now();
}

#[test]
fn typing_keeps_caret_and_updates_output() {
    let p = note(
        "d.md",
        "---\ntoolbook: 1\n---\n说明\n\n```hash name=b\nx\n```\n",
    );
    let mut app = start(&p);
    app.wait_for("text:x").unwrap();
    app.press("text:x").unwrap();
    app.press_key("end").unwrap();
    app.press_key("backspace").unwrap();
    keys(&mut app, "ab");
    // sha256("ab")
    app.wait_for("text:fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603")
        .unwrap();
    // The run that showed it must not have moved the caret out of the block.
    app.wait(Duration::from_millis(400)).unwrap();
    keys(&mut app, "c");
    // sha256("abc")
    app.wait_for("text:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        .unwrap();
    app.wait(Duration::from_millis(1200)).unwrap();
    assert!(
        std::fs::read_to_string(&p)
            .unwrap()
            .contains("```hash name=b\nabc\n```")
    );
}

#[test]
fn editing_settings_line_updates_info() {
    let p = note(
        "e.md",
        "---\ntoolbook: 1\n---\n```json name=a\n{\"a\":[5],\"b\":7}\n```\n```pick name=p input=$a path=a\n```\n",
    );
    let mut app = start(&p);
    see_part(&mut app, "5");
    app.type_text("id:block-info-pick", "").unwrap();
    app.press_key("end").unwrap();
    app.press_key("backspace").unwrap();
    app.type_text("id:block-info-pick", "b").unwrap();
    app.wait_for("text:7").unwrap();
    app.wait(Duration::from_millis(1200)).unwrap();
    assert!(
        std::fs::read_to_string(&p)
            .unwrap()
            .contains("```pick name=p input=$a path=b")
    );
}

#[test]
fn pressing_settings_line_edits_it_not_the_text() {
    let p = note(
        "set.md",
        "---\ntoolbook: 1\n---\n说明\n\n```http name=r\nGET http://127.0.0.1:9/\n```\n",
    );
    let mut app = start(&p);
    app.wait_for("text:说明").unwrap();
    // A double press: the editor used to take it, selecting a word of the
    // request, which the next key replaced.
    app.press("id:block-info").unwrap();
    app.press("id:block-info").unwrap();
    app.press_key("end").unwrap();
    app.press_key("x").unwrap();
    app.wait(Duration::from_millis(1200)).unwrap();
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(
        saved.contains("```http name=rx\nGET http://127.0.0.1:9/\n```"),
        "{saved}"
    );
}

#[derive(Clone, PartialEq, Props)]
struct ClosableProps {
    path: String,
}

/// The editor and a button that takes it away, as switching notes does.
#[component]
fn Closable(props: ClosableProps) -> impl IntoElement {
    let shown = signal(true);
    let path = props.path.clone();
    view()
        .flex_1()
        .child(text("关闭").on_press(move || shown.set(false)))
        .child(dynamic(move || {
            shown.get().then(|| {
                toolbook_app::editor::note_editor()
                    .path(path.clone())
                    .into_element()
            })
        }))
}

#[test]
fn closing_right_after_an_edit_keeps_it() {
    let p = note(
        "f.md",
        "---\ntoolbook: 1\n---\n说明\n\n```hash name=b\nx\n```\n",
    );
    let path = p.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<Closable>(move || ClosableProps {
            path: path.clone(),
        }))
    };
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:x").unwrap();
    app.press("text:x").unwrap();
    app.press_key("end").unwrap();
    keys(&mut app, "yz");
    // The edit is in (not saved yet: that takes 800ms) before the editor goes.
    app.wait_for("text:xyz").unwrap();
    app.press("text:关闭").unwrap();
    app.wait(Duration::from_millis(100)).unwrap();
    assert!(
        std::fs::read_to_string(&p)
            .unwrap()
            .contains("```hash name=b\nxyz\n```")
    );
}

#[test]
fn editing_a_block_keeps_tables_and_html() {
    let src = "---\ntoolbook: 1\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<div>html</div>\n\n```hash name=b\nx\n```\n";
    let p = note("g.md", src);
    let mut app = start(&p);
    app.wait_for("text:x").unwrap();
    app.press("text:x").unwrap();
    app.press_key("end").unwrap();
    keys(&mut app, "y");
    app.wait(Duration::from_millis(1200)).unwrap();
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(saved.contains("```hash name=b\nxy\n```"), "{saved}");
    assert!(saved.contains("| a | b |\n|---|---|\n| 1 | 2 |"), "{saved}");
    assert!(saved.contains("<div>html</div>"), "{saved}");
}

#[test]
fn a_block_that_stops_being_a_tool_shows_no_other_result() {
    let p = note(
        "h.md",
        "---\ntoolbook: 1\n---\n```json\n{\"k\":1}\n```\n```base64 name=b\naGVsbG8gd29ybGQ=\n```\n",
    );
    let mut app = start(&p);
    app.wait_for("text:hello world").unwrap();
    app.type_text("id:block-info-json", "c").unwrap(); // json → jsonc
    see_part(&mut app, "不是已知工具");
    app.wait(Duration::from_millis(400)).unwrap();
    let hello = app
        .screen()
        .texts()
        .iter()
        .filter(|t| t.contains("hello world"))
        .count();
    assert_eq!(hello, 1, "{:?}", app.screen().texts());
}

#[test]
fn no_note_shows_an_empty_state_not_an_editor() {
    let mut app = start(std::path::Path::new(""));
    app.wait_for("text:没有打开的笔记").unwrap();
    assert!(
        app.type_text("id:note-editor", "x").is_err(),
        "nothing to type into"
    );
}

#[test]
fn a_failed_save_is_shown() {
    let p = note("i.md", "---\ntoolbook: 1\n---\n```hash name=b\nx\n```\n");
    let mut app = start(&p);
    app.wait_for("text:x").unwrap();
    std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    app.press("text:x").unwrap();
    app.press_key("end").unwrap();
    keys(&mut app, "y");
    app.wait(Duration::from_millis(1200)).unwrap();
    see_part(&mut app, "保存失败");
}

#[test]
fn secret_is_typed_masked_and_never_saved() {
    let body = "---\ntoolbook: 1\n---\n说明\n\n```text name=body\n{\"event\":\"payment.succeeded\",\"order_id\":\"ORD20261029001\",\"amount\":\"128.00\",\"currency\":\"HKD\"}\n```\n\n```secret name=key\n```\n\n```hmac name=mac data=$body key=$key\n```\n";
    let p = note("j.md", body);
    let mut app = start(&p);
    see_part(&mut app, "缺少密钥");
    app.type_text("id:block-secret", "whsec_demo_9f3a1c")
        .unwrap();
    see_part(
        &mut app,
        "6386a16c3050225e373c9e7fd0b22034920ba0e698bed6e33420ba0c44d412f0",
    );
    // Edit elsewhere so the note is saved, then look at the file.
    app.press("text:说明").unwrap();
    app.press_key("end").unwrap();
    keys(&mut app, "x");
    app.wait(Duration::from_millis(1200)).unwrap();
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(saved.contains("说明x"), "{saved}");
    assert!(!saved.contains("whsec_demo"), "{saved}");
}

#[test]
fn a_note_renamed_while_open_is_not_written_back_to_its_old_path() {
    let p = note(
        "k.md",
        "---\ntoolbook: 1\n---\n说明\n\n```hash name=b\nx\n```\n",
    );
    let path = p.to_string_lossy().into_owned();
    let mut config = EngineConfig {
        viewport: Size::new(px(1000.0), px(900.0)),
        ..EngineConfig::new(Root::new::<Closable>(move || ClosableProps {
            path: path.clone(),
        }))
    };
    config.environment.os = Os::MacOs;
    config.environment.idiom = Idiom::Desktop;
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:x").unwrap();
    app.press("text:x").unwrap();
    app.press_key("end").unwrap();
    keys(&mut app, "yz");
    std::fs::rename(&p, p.with_file_name("renamed.md")).unwrap();
    app.press("text:关闭").unwrap();
    app.wait(Duration::from_millis(1200)).unwrap();
    assert!(!p.exists(), "no stray file at the old path");
}

#[test]
fn a_tool_block_in_a_plain_note_says_why_it_does_not_run() {
    // No `toolbook: 1`: an unnamed `base64` block is plain code; say so.
    let p = note("plain.md", "# 普通笔记\n\n```base64\naGVsbG8=\n```\n\n```rust\nfn main() {}\n```\n");
    let mut app = start(&p);
    see_part(&mut app, "toolbook: 1");
    // Code that names no tool gets no such line.
    let lines = app.screen().texts().iter().filter(|t| t.contains("toolbook: 1")).count();
    assert_eq!(lines, 1);
}

#[test]
fn a_block_reading_a_secret_does_not_show_it() {
    let body = "---\ntoolbook: 1\n---\n说明\n\n```secret name=key\n```\n\n```text name=echo input=$key\n```\n";
    let p = note("sec-echo.md", body);
    let mut app = start(&p);
    see_part(&mut app, "缺少密钥");
    app.type_text("id:block-secret", "whsec_echo_77").unwrap();
    see_part(&mut app, "••••••");
    let texts = app.screen().texts();
    // Only the masked field may hold it; no result spells it out.
    assert!(
        texts.iter().filter(|t| t.contains("whsec_echo_77")).count() <= 1,
        "{texts:?}"
    );
}
