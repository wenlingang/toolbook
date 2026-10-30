use serde_json::json;
use toolbook_core::Value;
use toolbook_core::fence::{VarRef, parse_ref};
use toolbook_core::path::{get_path, parse_path};
use toolbook_core::template::{render_template, template_refs};

#[test]
fn get_path_walks_json_and_text() {
    let v = Value::Json(json!({"a": {"b": [1, 2]}}));
    assert_eq!(get_path(&v, &parse_path("a.b[1]").unwrap()).unwrap(), Value::Json(json!(2)));
    let t = Value::Text("{\"x\":\"y\"}".into());
    assert_eq!(get_path(&t, &parse_path("x").unwrap()).unwrap(), Value::Text("y".into()));
    assert_eq!(get_path(&v, &[]).unwrap(), v);
    assert_eq!(get_path(&v, &parse_path("a.z").unwrap()).unwrap_err(), "字段 a.z 不存在");
}

#[test]
fn template_renders_refs_and_escapes() {
    let lookup = |r: &VarRef| -> Result<Value, String> {
        match r.name.as_str() {
            "tok" => Ok(Value::Text("abc".into())),
            "obj" => Ok(Value::Json(json!({"k": 1}))),
            _ => Err(format!("没有 {}", r.display())),
        }
    };
    assert_eq!(
        render_template("Bearer {{ tok }} {{\"{{\"}} {{obj}}", &lookup).unwrap(),
        "Bearer abc {{ {\"k\":1}"
    );
    assert!(render_template("{{ nope }}", &lookup).unwrap_err().contains("$nope"));
    let refs = template_refs("{{a.b}} {{ c }} {{\"{{\"}}");
    assert_eq!(refs, vec![parse_ref("$a.b").unwrap(), parse_ref("$c").unwrap()]);
}
