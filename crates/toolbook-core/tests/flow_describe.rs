use std::fs;

use serde_json::json;
use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockStatus, RunOptions};
use toolbook_core::{Value, describe, run_file};

const HEAD: &str = "---\ntoolbook: 1\n---\n";

fn dir() -> std::path::PathBuf {
    // Tests run in parallel: a counter keeps their directories apart even
    // when the clock reads the same.
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("toolbook-test-{}-{}-{n}", std::process::id(), rand_suffix()));
    fs::create_dir_all(&d).unwrap();
    d
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
}

#[test]
fn flow_passes_inputs_and_returns_out() {
    let d = dir();
    fs::write(d.join("double.md"), format!("{HEAD}```text name=n param\n1\n```\n```js name=r\nNumber($n) * 2\n```\n")).unwrap();
    fs::write(d.join("main.md"), format!("{HEAD}```flow name=x src=double.md n=21\n```\n")).unwrap();
    let r = run_file(&d.join("main.md"), &RunOptions::default()).unwrap();
    assert_eq!(r.blocks[0].status, BlockStatus::Ok(Value::Json(json!(42))));
}

#[test]
fn flow_child_error_names_block() {
    let d = dir();
    fs::write(d.join("bad.md"), format!("{HEAD}```json name=j\n{{oops\n```\n")).unwrap();
    fs::write(d.join("main.md"), format!("{HEAD}```flow name=x src=bad.md\n```\n")).unwrap();
    let r = run_file(&d.join("main.md"), &RunOptions::default()).unwrap();
    assert!(matches!(&r.blocks[0].status, BlockStatus::Error(m) if m.contains("bad.md") && m.contains("[1]")), "{:?}", r.blocks[0].status);
}

#[test]
fn flow_recursion_is_detected() {
    let d = dir();
    fs::write(d.join("a.md"), format!("{HEAD}```flow name=x src=b.md\n```\n")).unwrap();
    fs::write(d.join("b.md"), format!("{HEAD}```flow name=y src=a.md\n```\n")).unwrap();
    let r = run_file(&d.join("a.md"), &RunOptions::default()).unwrap();
    assert!(matches!(&r.blocks[0].status, BlockStatus::Error(m) if m.contains("递归调用")), "{:?}", r.blocks[0].status);
}

#[test]
fn flow_finds_notes_in_library() {
    let lib = dir();
    fs::create_dir_all(lib.join("工具")).unwrap();
    fs::write(lib.join("工具/up.md"), format!("{HEAD}```text name=s param\n```\n```js name=r\n$s.toUpperCase()\n```\n")).unwrap();
    let elsewhere = dir();
    fs::write(elsewhere.join("main.md"), format!("{HEAD}```flow name=x src=工具/up.md s=abc\n```\n")).unwrap();
    let opts = RunOptions { library: lib, ..RunOptions::default() };
    let r = run_file(&elsewhere.join("main.md"), &opts).unwrap();
    assert_eq!(r.blocks[0].status, BlockStatus::Ok(Value::Text("ABC".into())));
}

#[test]
fn describe_lists_params_secrets_and_outputs() {
    let doc = Document::parse(&format!("{HEAD}把 CSV 转成 SQL。\n\n```text name=csv param\na,b\n```\n```secret name=key\n```\n```to-sql name=sql input=$csv table=t\n```\n```text\nunnamed\n```\n"));
    let spec = describe(&doc);
    assert_eq!(spec.description.as_deref(), Some("把 CSV 转成 SQL。"));
    assert_eq!(spec.inputs.len(), 2);
    assert_eq!((spec.inputs[0].name.as_str(), spec.inputs[0].default.as_str(), spec.inputs[0].secret), ("csv", "a,b", false));
    assert_eq!((spec.inputs[1].name.as_str(), spec.inputs[1].secret), ("key", true));
    assert_eq!(spec.outputs, ["csv", "key", "sql"]);
}

#[test]
fn flow_unknown_input_is_error() {
    let d = dir();
    fs::write(d.join("double.md"), format!("{HEAD}```text name=n param\n1\n```\n```js name=r\nNumber($n) * 2\n```\n")).unwrap();
    fs::write(d.join("main.md"), format!("{HEAD}```flow name=x src=double.md nn=99\n```\n")).unwrap();
    let r = run_file(&d.join("main.md"), &RunOptions::default()).unwrap();
    assert!(matches!(&r.blocks[0].status, BlockStatus::Error(m) if m.contains("nn") && m.contains("n")), "{:?}", r.blocks[0].status);
}
