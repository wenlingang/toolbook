use std::path::Path;

use toolbook_core::Value;
use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockStatus, RunOptions, run};

fn note(body: &str) -> Document {
    Document::parse(&format!("---\ntoolbook: 1\n---\n{body}"))
}

fn go(body: &str, opts: &RunOptions) -> Vec<BlockStatus> {
    run(&note(body), opts, Path::new(".")).blocks.into_iter().map(|b| b.status).collect()
}

#[test]
fn downstream_of_error_is_upstream_error() {
    let s = go("```text name=a\n```\n```text name=b input=$a\n```\n", &RunOptions::default());
    assert!(matches!(s[0], BlockStatus::Incomplete(_)));
    assert_eq!(s[1], BlockStatus::Upstream { block: 0, name: "a".into() });
}

#[test]
fn input_attr_passes_value_down() {
    let s = go("```text name=a\nhello\n```\n```text name=b input=$a\n```\n", &RunOptions::default());
    assert_eq!(s[1], BlockStatus::Ok(Value::Text("hello".into())));
}

#[test]
fn forward_and_missing_refs_are_errors() {
    let s = go("```text name=b input=$a\n```\n```text name=a\nx\n```\n", &RunOptions::default());
    match &s[0] {
        BlockStatus::Error(m) => assert!(m.contains("$a"), "{m}"),
        other => panic!("{other:?}"),
    }
    let s = go("```text input=$nope\n```\n", &RunOptions::default());
    assert!(matches!(&s[0], BlockStatus::Error(m) if m.contains("$nope")));
}

#[test]
fn duplicate_names_are_errors() {
    let s = go("```text name=a\nx\n```\n```text name=a\ny\n```\n", &RunOptions::default());
    assert!(matches!(&s[1], BlockStatus::Error(m) if m.contains("a") && m.contains("重复")));
}

#[test]
fn unknown_tool_is_error() {
    let s = go("```nosuch name=a\nx\n```\n", &RunOptions::default());
    assert!(matches!(&s[0], BlockStatus::Error(m) if m.contains("nosuch")));
}

#[test]
fn param_override_replaces_content() {
    let mut opts = RunOptions::default();
    opts.overrides.insert("h".into(), "y".into());
    let s = go("```text name=h param\nx\n```\n", &opts);
    assert_eq!(s[0], BlockStatus::Ok(Value::Text("y".into())));
}

#[test]
fn secret_from_env_and_never_from_content() {
    let body = "```secret name=key\nleak\n```\n";
    let s = go(body, &RunOptions::default());
    assert!(matches!(&s[0], BlockStatus::Incomplete(m) if m.contains("TOOLBOOK_SECRET_KEY")));
    let mut opts = RunOptions::default();
    opts.env.insert("TOOLBOOK_SECRET_KEY".into(), "s".into());
    assert_eq!(go(body, &opts)[0], BlockStatus::Ok(Value::Text("s".into())));
}

#[test]
fn output_defaults_to_last_named_block() {
    let doc = note("```text name=a\nx\n```\n```text name=b\ny\n```\n```text\nz\n```\n");
    let r = run(&doc, &RunOptions::default(), Path::new("."));
    assert_eq!(r.output(None).unwrap().name.as_deref(), Some("b"));
    assert_eq!(r.output(Some("a")).unwrap().index, 0);
}

#[test]
fn unquoted_jsonpath_is_literal() {
    let s = go("```json name=a\n{\"x\":[{\"b\":1},{\"b\":2}]}\n```\n```pick input=$a path=$.x[*].b\n```\n", &RunOptions::default());
    assert_eq!(s[1], BlockStatus::Ok(Value::Json(serde_json::json!([1, 2]))));
}

#[test]
fn malformed_ref_path_is_error_not_truncated() {
    let s = go("```json name=a\n{\"k\":[1,2]}\n```\n```text input=$a.k[1\n```\n", &RunOptions::default());
    assert!(matches!(&s[1], BlockStatus::Error(m) if m.contains("$a.k[1")), "{:?}", s[1]);
    let s = go("```json name=a\n{\"k\":[1,2]}\n```\n```pick input=$a path=k[0]b\n```\n", &RunOptions::default());
    assert!(matches!(&s[1], BlockStatus::Error(m) if m.contains("k[0]b")), "{:?}", s[1]);
}

#[test]
fn memo_reuses_unchanged_blocks_and_recomputes_changed() {
    use toolbook_core::engine::Memo;
    let memo = Memo::default();
    let opts = RunOptions { memo: Some(memo.clone()), ..RunOptions::default() };
    let body = "```text name=a\nx\n```\n```text name=b input=$a\n```\n";
    run(&note(body), &opts, Path::new("."));
    assert_eq!(memo.len(), 2);
    // Plant a value for block b: a rerun with nothing changed must reuse it.
    memo.overwrite_all_for_test("b", Value::Text("from memo".into()));
    let s = go(body, &opts);
    assert_eq!(s[1], BlockStatus::Ok(Value::Text("from memo".into())));
    // Change a: b depends on it, so it is computed again.
    let s = go("```text name=a\ny\n```\n```text name=b input=$a\n```\n", &opts);
    assert_eq!(s[1], BlockStatus::Ok(Value::Text("y".into())));
}

#[test]
fn memo_keeps_manual_results_for_later_runs() {
    use toolbook_core::engine::Memo;
    let memo = Memo::default();
    // A manual block (http) that has not run stays NotRun without a memo entry.
    let body = "```http name=r\nGET http://127.0.0.1:9/\n```\n";
    let opts = RunOptions { memo: Some(memo.clone()), ..RunOptions::default() };
    assert_eq!(go(body, &opts)[0], BlockStatus::NotRun);
    memo.overwrite_all_for_test("r", Value::Text("cached response".into()));
    assert_eq!(go(body, &opts)[0], BlockStatus::Ok(Value::Text("cached response".into())));
}

#[test]
fn what_reads_a_request_not_run_yet_waits_for_it() {
    // Not an error: ⌘↵ runs the request, and what reads it with it.
    let doc = Document::parse("---\ntoolbook: 1\n---\n```http name=r\nGET https://example.com\n```\n```js name=x\n$r.status\n```\n");
    let report = run(&doc, &RunOptions::default(), Path::new("."));
    assert_eq!(report.blocks[0].status, BlockStatus::NotRun);
    assert_eq!(report.blocks[1].status, BlockStatus::NotRun);
}

#[test]
fn js_reading_the_clock_or_random_is_not_memoized() {
    use toolbook_core::engine::Memo;
    let memo = Memo::default();
    let opts = RunOptions { memo: Some(memo.clone()), ..RunOptions::default() };
    for code in ["Date.now()", "Math.random()"] {
        let body = format!("```js name=t\n{code}\n```\n```js name=u\n$t + 1\n```\n");
        go(&body, &opts);
        memo.overwrite_all_for_test("t", Value::Text("stale".into()));
        memo.overwrite_all_for_test("u", Value::Text("stale".into()));
        let s = go(&body, &opts);
        assert!(matches!(&s[0], BlockStatus::Ok(Value::Json(_))), "{code}: {:?}", s[0]);
        assert!(matches!(&s[1], BlockStatus::Ok(Value::Json(_))), "{code}: {:?}", s[1]);
    }
    // Other js stays memoized.
    let body = "```js name=v\n1 + 1\n```\n";
    go(body, &opts);
    memo.overwrite_all_for_test("v", Value::Text("kept".into()));
    assert_eq!(go(body, &opts)[0], BlockStatus::Ok(Value::Text("kept".into())));
}

#[test]
fn memo_stays_bounded_while_typing() {
    use toolbook_core::engine::Memo;
    let memo = Memo::default();
    let opts = RunOptions { memo: Some(memo.clone()), ..RunOptions::default() };
    for i in 0..1000 {
        go(&format!("```text name=a\n{i}\n```\n```text name=b input=$a\n```\n"), &opts);
    }
    assert!(memo.len() <= 300, "{}", memo.len());
    // What the latest run used is still kept.
    memo.overwrite_all_for_test("b", Value::Text("kept".into()));
    let s = go("```text name=a\n999\n```\n```text name=b input=$a\n```\n", &opts);
    assert_eq!(s[1], BlockStatus::Ok(Value::Text("kept".into())));
}

#[test]
fn a_cancelled_run_stops_between_blocks() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use toolbook_core::engine::Memo;
    let memo = Memo::default();
    let cancel = Arc::new(AtomicBool::new(true));
    let opts = RunOptions { memo: Some(memo.clone()), cancel: Some(cancel.clone()), ..RunOptions::default() };
    let report = run(&note("```text name=a\nx\n```\n```text name=b input=$a\n```\n"), &opts, Path::new("."));
    assert!(report.cancelled);
    assert!(report.blocks.is_empty());
    assert!(memo.is_empty());
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    let report = run(&note("```text name=a\nx\n```\n"), &opts, Path::new("."));
    assert!(!report.cancelled);
    assert_eq!(report.blocks.len(), 1);
}

#[test]
fn masked_report_blanks_secret_values_out_of_what_shows() {
    let mut opts = RunOptions::default();
    opts.overrides.insert("key".into(), "s3cr3t-value".into());
    let body = "```secret name=key\n```\n```text name=a input=$key\n```\n```js name=j\n({k: $key, n: 1})\n```\n```hmac name=h key=$key\nbody\n```\n";
    let report = run(&note(body), &opts, Path::new(".")).masked();
    assert_eq!(report.blocks[1].status, BlockStatus::Ok(Value::Text("••••••".into())));
    match &report.blocks[2].status {
        BlockStatus::Ok(Value::Json(j)) => assert_eq!(j, &serde_json::json!({"k": "••••••", "n": 1})),
        other => panic!("{other:?}"),
    }
    // One-way results carry no value to blank out.
    match &report.blocks[3].status {
        BlockStatus::Ok(Value::Json(j)) => assert!(!j.to_string().contains('•'), "{j}"),
        other => panic!("{other:?}"),
    }
}
