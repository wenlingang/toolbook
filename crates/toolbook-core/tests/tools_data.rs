mod common;

use std::path::Path;

use common::last;
use serde_json::json;
use toolbook_core::doc::Document;
use toolbook_core::engine::{BlockStatus, RunOptions, run};
use toolbook_core::{Table, Value};

fn ok(s: BlockStatus) -> Value {
    match s {
        BlockStatus::Ok(v) => v,
        other => panic!("not ok: {other:?}"),
    }
}

const SPANS: &str = r#"```text name=t
{"spans":[
 {"service":"gateway","name":"POST /v1/orders","start":0,"duration":312},
 {"service":"order-api","name":"CreateOrder","start":8,"duration":290},
 {"service":"risk-check","name":"CheckLimit","start":20,"duration":44},
 {"service":"quote-svc","name":"GetQuote","start":66,"duration":52},
 {"service":"mysql","name":"INSERT orders","start":122,"duration":164},
 {"service":"redis","name":"SET order:cache","start":288,"duration":6}
]}
```
"#;

#[test]
fn csv_with_bom_and_chinese_headers() {
    let v = ok(last("```csv\n\u{feff}姓名,手机,城市\n张三,13800138000,深圳\n```\n"));
    assert_eq!(
        v,
        Value::Table(Table {
            columns: vec!["姓名".into(), "手机".into(), "城市".into()],
            rows: vec![vec!["张三".into(), "13800138000".into(), "深圳".into()]],
        })
    );
}

#[test]
fn scenario3_cleaning() {
    let body = r#"---
toolbook: 1
---
```csv name=rows dedupe=手机 require="手机~^1\d{10}$"
姓名,手机,城市
张三,13800138000,深圳
李四,1390013,广州
王五,13700137000,香港
张三,13800138000,深圳
赵六,15900159000,上海
```
```map name=clean input=$rows rename="姓名:name,手机:phone,城市:city"
```
```to-json name=json input=$clean
```
"#;
    let r = run(&Document::parse(body), &RunOptions::default(), Path::new("."));
    let notes = &r.blocks[0].notes;
    assert!(notes.iter().any(|n| n.contains("第 3 行") && n.contains("手机")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("第 5 行") && n.contains("第 2 行")), "{notes:?}");
    assert_eq!(
        r.blocks[2].status,
        BlockStatus::Ok(Value::Json(json!([
            {"name":"张三","phone":"13800138000","city":"深圳"},
            {"name":"王五","phone":"13700137000","city":"香港"},
            {"name":"赵六","phone":"15900159000","city":"上海"}
        ])))
    );
}

#[test]
fn to_sql_insert_escapes_quotes() {
    let body = "```json name=rows\n[{\"name\":\"O'Neil\",\"age\":3,\"x\":null}]\n```\n```to-sql input=$rows table=users\n```\n";
    assert_eq!(
        ok(last(body)),
        Value::Text("INSERT INTO users (name, age, x) VALUES ('O''Neil', 3, NULL);".into())
    );
    let upd = "```json name=rows\n[{\"id\":7,\"name\":\"a\"}]\n```\n```to-sql input=$rows table=users mode=update key=id\n```\n";
    assert_eq!(ok(last(upd)), Value::Text("UPDATE users SET name = 'a' WHERE id = 7;".into()));
    assert!(matches!(last("```json name=r\n[]\n```\n```to-sql input=$r\n```\n"), BlockStatus::Incomplete(m) if m.contains("table=")));
}

#[test]
fn filter_numeric_compare() {
    let body = format!("{SPANS}```pick name=spans input=$t path=spans\n```\n```filter input=$spans where=\"duration > 100\"\n```\n");
    match ok(last(&body)) {
        Value::Json(j) => {
            let names: Vec<_> = j.as_array().unwrap().iter().map(|s| s["service"].as_str().unwrap().to_owned()).collect();
            assert_eq!(names, ["gateway", "order-api", "mysql"]);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(last("```json name=a\n[]\n```\n```filter input=$a where=\"oops\"\n```\n"), BlockStatus::Error(m) if m.contains("duration > 100")));
}

#[test]
fn pick_jsonpath_and_dotted() {
    let body = format!("{SPANS}```pick input=$t path=\"$.spans[*].service\"\n```\n");
    assert_eq!(ok(last(&body)), Value::Json(json!(["gateway","order-api","risk-check","quote-svc","mysql","redis"])));
    let body = format!("{SPANS}```pick input=$t path=spans[4].duration\n```\n");
    assert_eq!(ok(last(&body)), Value::Json(json!(164)));
}

#[test]
fn json_error_has_line_col() {
    assert!(matches!(last("```json\n{\"a\":}\n```\n"), BlockStatus::Error(m) if m.contains("第 1 行")));
    assert_eq!(ok(last("```json mode=minify\n{ \"a\" : 1 }\n```\n")), Value::Text("{\"a\":1}".into()));
    assert!(matches!(last("```json\n\n```\n"), BlockStatus::Incomplete(_)));
}

#[test]
fn to_sql_quotes_identifiers_that_need_it() {
    let body = "```json name=rows\n[{\"first name\":\"a\",\"order\":1,\"id\":2}]\n```\n```to-sql input=$rows table=t\n```\n";
    assert_eq!(ok(last(body)), Value::Text("INSERT INTO t (`first name`, `order`, id) VALUES ('a', 1, 2);".into()));
    let pg = "```json name=rows\n[{\"first name\":\"a\"}]\n```\n```to-sql input=$rows table=\"my table\" dialect=postgres\n```\n";
    assert_eq!(ok(last(pg)), Value::Text("INSERT INTO \"my table\" (\"first name\") VALUES ('a');".into()));
}

#[test]
fn to_sql_escapes_backslashes_for_mysql() {
    // In MySQL a backslash escapes the quote after it: doubled, it cannot end the string early.
    let body = "```json name=rows\n[{\"v\":\"a\\\\' OR 1=1 -- \"}]\n```\n```to-sql input=$rows table=t\n```\n";
    assert_eq!(ok(last(body)), Value::Text("INSERT INTO t (v) VALUES ('a\\\\'' OR 1=1 -- ');".into()));
    let pg = "```json name=rows\n[{\"v\":\"a\\\\b\"}]\n```\n```to-sql input=$rows table=t dialect=postgres\n```\n";
    assert_eq!(ok(last(pg)), Value::Text("INSERT INTO t (v) VALUES ('a\\b');".into()), "standard SQL keeps backslashes as they are");
}
