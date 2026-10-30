use serde_json::json;
use toolbook_core::{Table, Value};

#[test]
fn value_summary_truncates_and_tables_describe_shape() {
    let long = Value::Text("a".repeat(100)).summary();
    assert_eq!(long.chars().count(), 81);
    assert!(long.ends_with('…'));
    let table = Table {
        columns: vec!["a".into(), "b".into()],
        rows: vec![vec!["1".into(), "2".into()]; 3],
    };
    assert_eq!(Value::Table(table).summary(), "表格 · 3 行 × 2 列");
    assert_eq!(Value::Json(json!({"a": 1})).summary(), "{\"a\":1}");
}

#[test]
fn table_to_json_and_plain_csv() {
    let table = Table {
        columns: vec!["name".into(), "city".into()],
        rows: vec![vec!["张三".into(), "深圳".into()]],
    };
    let v = Value::Table(table);
    assert_eq!(v.to_json(), json!([{"name": "张三", "city": "深圳"}]));
    assert_eq!(v.render_plain(), "name,city\n张三,深圳\n");
    assert_eq!(Value::Json(json!({"a": 1})).render_plain(), "{\n  \"a\": 1\n}");
}
