//! `diff a=… b=…`: field by field for JSON, line by line for text.

use serde_json::{Value as Json, json};
use similar::TextDiff;

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Diff;

fn as_json(v: &Value) -> Option<Json> {
    match v {
        Value::Text(s) => {
            let t = s.trim_start();
            (t.starts_with('{') || t.starts_with('[')).then(|| serde_json::from_str(s).ok()).flatten()
        }
        other => Some(other.to_json()),
    }
}

fn join(base: &str, seg: &str) -> String {
    if base.is_empty() { seg.to_owned() } else { format!("{base}.{seg}") }
}

fn walk(path: &str, a: &Json, b: &Json, out: &mut Vec<Json>) {
    match (a, b) {
        (Json::Object(x), Json::Object(y)) => {
            for (k, va) in x {
                match y.get(k) {
                    Some(vb) => walk(&join(path, k), va, vb, out),
                    None => out.push(json!({"path": join(path, k), "op": "removed", "left": va, "right": null})),
                }
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    out.push(json!({"path": join(path, k), "op": "added", "left": null, "right": vb}));
                }
            }
        }
        (Json::Array(x), Json::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let p = format!("{path}[{i}]");
                match (x.get(i), y.get(i)) {
                    (Some(va), Some(vb)) => walk(&p, va, vb, out),
                    (Some(va), None) => out.push(json!({"path": p, "op": "removed", "left": va, "right": null})),
                    (None, Some(vb)) => out.push(json!({"path": p, "op": "added", "left": null, "right": vb})),
                    (None, None) => {}
                }
            }
        }
        _ if a != b => out.push(json!({"path": path, "op": "changed", "left": a, "right": b})),
        _ => {}
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        other => other.render_plain(),
    }
}

impl Tool for Diff {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let a = match ctx.attrs.get("a") {
            Some(v) => v.clone(),
            None => ctx.require_primary()?.clone(),
        };
        let b = ctx.attrs.get("b").cloned().ok_or_else(|| ToolError::Incomplete("请设置 b=".into()))?;
        if let (Some(ja), Some(jb)) = (as_json(&a), as_json(&b)) {
            let mut changes = Vec::new();
            walk("", &ja, &jb, &mut changes);
            if changes.is_empty() {
                return Ok(Value::Text("无差异".into()));
            }
            return Ok(Value::Json(json!({"kind": "json", "changes": changes})));
        }
        let (ta, tb) = (text_of(&a), text_of(&b));
        if ta == tb {
            return Ok(Value::Text("无差异".into()));
        }
        let diff = TextDiff::from_lines(&ta, &tb);
        Ok(Value::Text(diff.unified_diff().context_radius(3).header("a", "b").to_string()))
    }
}
