mod common;

use std::io::{Read, Write};
use std::net::TcpListener;

use common::{last, run_body};
use serde_json::json;
use toolbook_core::Value;
use toolbook_core::engine::{BlockStatus, RunOptions};

fn ok(s: BlockStatus) -> Value {
    match s {
        BlockStatus::Ok(v) => v,
        other => panic!("not ok: {other:?}"),
    }
}

fn ts_at(input: &str, now_ms: i64) -> serde_json::Value {
    let opts = RunOptions { now_ms, ..RunOptions::default() };
    match run_body(&format!("```timestamp\n{input}\n```\n"), &opts).pop().unwrap() {
        BlockStatus::Ok(Value::Json(j)) => j,
        other => panic!("{other:?}"),
    }
}

#[test]
fn timestamp_seconds_and_millis() {
    let now = 1_761_703_600_000 + 3 * 3600 * 1000 + 20 * 60 * 1000;
    let a = ts_at("1761703600", now);
    let b = ts_at("1761703600000", now);
    assert_eq!(a, b);
    assert_eq!(a["unix"], json!(1761703600));
    assert_eq!(a["utc"], json!("2025-10-29 02:06:40"));
    assert_eq!(a["local"], json!("2025-10-29 10:06:40"));
    assert_eq!(a["relative"], json!("3 小时 20 分钟前"));
    let future = ts_at("1761703600", 1_761_703_600_000 - 2 * 86_400_000 - 3600 * 1000);
    assert_eq!(future["relative"], json!("2 天 1 小时后"));
    assert_eq!(ts_at("2025-10-29T02:06:40Z", now)["unix"], json!(1761703600));
    assert!(matches!(last("```timestamp\nhello\n```\n"), BlockStatus::Error(_)));
}

#[test]
fn timestamp_out_of_range_is_an_error_not_a_panic() {
    for n in [i64::MIN, i64::MAX, -9_223_372_036_854_775] {
        assert!(matches!(last(&format!("```timestamp\n{n}\n```\n")), BlockStatus::Error(_)), "{n}");
    }
}

#[test]
fn regex_extract_groups_and_replace() {
    assert_eq!(
        ok(last("```regex pattern=\"(\\w+)=(\\d+)\"\na=1 b=2\n```\n")),
        Value::Json(json!([["a", "1"], ["b", "2"]]))
    );
    assert_eq!(ok(last("```regex pattern=\"\\d+\"\na1b22\n```\n")), Value::Json(json!(["1", "22"])));
    assert_eq!(
        ok(last("```regex pattern=\"(\\d+)\" mode=replace replace=\"<$1>\"\na1b22\n```\n")),
        Value::Text("a<1>b<22>".into())
    );
    assert!(matches!(last("```regex pattern=\"(\"\nx\n```\n"), BlockStatus::Error(_)));
}

#[test]
fn json_diff_paths() {
    let body = "```json name=a\n{\"name\":\"张三\",\"city\":\"广州\"}\n```\n```json name=b\n{\"name\":\"张三\",\"city\":\"深圳\",\"x\":1}\n```\n```diff a=$a b=$b\n```\n";
    assert_eq!(
        ok(last(body)),
        Value::Json(json!({"kind":"json","changes":[
            {"path":"city","op":"changed","left":"广州","right":"深圳"},
            {"path":"x","op":"added","left":null,"right":1}
        ]}))
    );
    let text = "```text name=a\nx\ny\n```\n```text name=b\nx\nz\n```\n```diff a=$a b=$b\n```\n";
    match ok(last(text)) {
        Value::Text(t) => assert!(t.contains("-y") && t.contains("+z"), "{t}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(ok(last("```text name=a\nx\n```\n```diff a=$a b=$a\n```\n")), Value::Text("无差异".into()));
}

#[test]
fn js_uses_upstream_vars() {
    assert_eq!(ok(last("```text name=n\n3\n```\n```js name=r\nNumber($n) * 2\n```\n")), Value::Json(json!(6)));
    assert_eq!(
        ok(last("```json name=rows\n[{\"a\":1},{\"a\":5}]\n```\n```js\n$rows.filter(r => r.a > 2)\n```\n")),
        Value::Json(json!([{"a": 5}]))
    );
    assert_eq!(ok(last("```js\n'hi'\n```\n")), Value::Text("hi".into()));
    assert!(matches!(last("```js\nwhile(true){}\n```\n"), BlockStatus::Error(m) if m.contains("超过 1 秒")));
    assert!(matches!(last("```js\n(((\n```\n"), BlockStatus::Error(_)));
}

#[test]
fn http_is_manual_and_hits_local_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = s.read(&mut buf).unwrap();
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let body = "{\"ok\":true}";
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        req
    });
    let body = format!("```text name=tok\nabc\n```\n```http name=resp\nGET http://127.0.0.1:{port}/x\nAuthorization: Bearer {{{{ tok }}}}\n```\n```pick input=$resp path=body.ok\n```\n");
    let lazy = run_body(&body, &RunOptions::default());
    assert_eq!(lazy[1], BlockStatus::NotRun);
    let opts = RunOptions { run_manual: true, ..RunOptions::default() };
    let r = run_body(&body, &opts);
    match &r[1] {
        BlockStatus::Ok(Value::Json(j)) => assert_eq!(j["status"], json!(200)),
        other => panic!("{other:?}"),
    }
    assert_eq!(r[2], BlockStatus::Ok(Value::Json(json!(true))));
    let req = server.join().unwrap();
    assert!(req.to_lowercase().contains("authorization: bearer abc"), "{req}");
}

#[test]
fn run_manual_always_sends_again_even_with_memo() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use toolbook_core::engine::Memo;
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
            let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        }
    });
    let body = format!("```http name=r\nGET http://127.0.0.1:{port}/\n```\n");
    let memo = Memo::default();
    let manual = RunOptions { run_manual: true, memo: Some(memo.clone()), ..RunOptions::default() };
    run_body(&body, &manual);
    run_body(&body, &manual);
    assert_eq!(hits.load(Ordering::SeqCst), 2, "each ⌘↵ sends the request");
    // An automatic run reuses the last response.
    let auto = RunOptions { memo: Some(memo), ..RunOptions::default() };
    assert!(matches!(run_body(&body, &auto)[0], BlockStatus::Ok(_)));
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[test]
fn http_says_how_to_write_a_request_before_it_is_run() {
    // Settings in the fence line instead of a request: said at once, not
    // only after ⌘↵.
    let body = "```js name=params\n({ a: 1 })\n```\n```http body=params input=$params\n```\n";
    match &run_body(body, &RunOptions::default())[1] {
        BlockStatus::Error(m) => assert!(m.contains("body、input") && m.contains("{{ params }}"), "{m}"),
        other => panic!("{other:?}"),
    }
    // No request yet: what to write.
    match &run_body("```http\n```\n", &RunOptions::default())[0] {
        BlockStatus::Incomplete(m) => assert!(m.contains("POST https://"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn js_errors_say_where_without_engine_stack() {
    let err = |code: &str| match last(&format!("```js\n{code}\n```\n")) {
        BlockStatus::Error(m) => m,
        other => panic!("{other:?}"),
    };
    assert_eq!(err("null.x"), "JS 出错：cannot read property 'x' of null（第 1 行第 1 列）");
    assert_eq!(err("1;\nthrow new Error(\"坏了\")"), "JS 出错：坏了（第 2 行第 11 列）");
    assert_eq!(err("function f(){ return g() }; f()"), "JS 出错：g is not defined（第 1 行第 22 列）");
    assert_eq!(err("syntax error here("), "JS 出错：expecting ';'（第 1 行第 8 列）");
    assert_eq!(err("throw 42"), "JS 出错：42");
}
