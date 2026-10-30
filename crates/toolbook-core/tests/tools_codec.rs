mod common;

use common::{b64url, last};
use serde_json::json;
use toolbook_core::Value;
use toolbook_core::engine::BlockStatus;

fn ok(s: BlockStatus) -> Value {
    match s {
        BlockStatus::Ok(v) => v,
        other => panic!("not ok: {other:?}"),
    }
}

#[test]
fn hmac_matches_openssl() {
    let body = r#"```text name=body
{"event":"payment.succeeded","order_id":"ORD20261029001","amount":"128.00","currency":"HKD"}
```
```text name=key
whsec_demo_9f3a1c
```
```hmac name=mac data=$body key=$key
```
"#;
    assert_eq!(
        ok(last(body)),
        Value::Json(json!({
            "hex": "6386a16c3050225e373c9e7fd0b22034920ba0e698bed6e33420ba0c44d412f0",
            "base64": "Y4ahbDBQIl43PJ5/0LIgNJILoOaYvtbjNCC6DETUEvA="
        }))
    );
}

#[test]
fn base64_decodes_chinese() {
    assert_eq!(ok(last("```base64\n5L2g5aW977yM5LiW55WM\n```\n")), Value::Text("你好，世界".into()));
    assert_eq!(ok(last("```base64\nSGk=\n```\n")), Value::Text("Hi".into()));
    assert_eq!(ok(last("```base64\nSGk\n```\n")), Value::Text("Hi".into()));
    assert!(matches!(last("```base64\nS\n```\n"), BlockStatus::Incomplete(m) if m.contains("不完整")));
    assert_eq!(ok(last("```base64 mode=encode\nHi\n```\n")), Value::Text("SGk=".into()));
}

#[test]
fn base64_binary_is_error_not_garbage() {
    assert!(matches!(last("```base64\n/w==\n```\n"), BlockStatus::Error(m) if m.contains("不是 UTF-8")));
}

#[test]
fn jwt_extracts_from_header_line() {
    let token = format!(
        "{}.{}.sig",
        b64url(r#"{"alg":"HS256","typ":"JWT"}"#),
        b64url(r#"{"sub":"u_10086","exp":1761703600}"#)
    );
    let body = format!("```jwt-decode\nAuthorization: Bearer {token}\n```\n");
    match ok(last(&body)) {
        Value::Json(j) => {
            assert_eq!(j["payload"]["exp"], json!(1761703600));
            assert_eq!(j["header"]["alg"], json!("HS256"));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(last("```jwt-decode\nno token here\n```\n"), BlockStatus::Incomplete(m) if m.contains("JWT")));
}

#[test]
fn hash_and_url() {
    assert_eq!(
        ok(last("```hash\nabc\n```\n")),
        Value::Text("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into())
    );
    assert_eq!(ok(last("```hash algo=md5\nabc\n```\n")), Value::Text("900150983cd24fb0d6963f7d28e17f72".into()));
    assert_eq!(ok(last("```url\na%20b+c\n```\n")), Value::Text("a b c".into()));
    assert_eq!(ok(last("```url mode=encode\na b/中\n```\n")), Value::Text("a%20b%2F%E4%B8%AD".into()));
}

#[test]
fn jwt_skips_hostnames_before_token() {
    let token = format!("{}.{}.sig", b64url(r#"{"alg":"HS256"}"#), b64url(r#"{"exp":1}"#));
    let body = format!("```jwt-decode\nHost: api.example.com\nAuthorization: Bearer {token}\n```\n");
    match ok(last(&body)) {
        Value::Json(j) => assert_eq!(j["payload"]["exp"], json!(1)),
        other => panic!("{other:?}"),
    }
}
