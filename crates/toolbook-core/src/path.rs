//! Taking a path out of a value: `$jwt.payload.exp`.

use serde_json::Value as Json;

pub use crate::fence::parse_path;
use crate::fence::{PathSeg, display_path};
use crate::value::Value;

/// The part of `v` at `path`. Text is read as JSON when a path is given;
/// a string result is text, anything else JSON.
pub fn get_path(v: &Value, path: &[PathSeg]) -> Result<Value, String> {
    if path.is_empty() {
        return Ok(v.clone());
    }
    // Walks JSON in place: only what the path reaches is copied.
    let owned;
    let root: &Json = match v {
        Value::Json(j) => j,
        Value::Text(s) => {
            owned = serde_json::from_str::<Json>(s)
                .map_err(|_| format!("文本不是 JSON，不能取字段 {}", display_path(path).trim_start_matches('.')))?;
            &owned
        }
        other => {
            owned = other.to_json();
            &owned
        }
    };
    let mut cur = root;
    for (i, seg) in path.iter().enumerate() {
        let next = match seg {
            PathSeg::Key(k) => cur.get(k.as_str()),
            PathSeg::Index(n) => cur.get(*n),
        };
        cur = next.ok_or_else(|| {
            format!("字段 {} 不存在", display_path(&path[..=i]).trim_start_matches('.'))
        })?;
    }
    Ok(match cur {
        Json::String(s) => Value::Text(s.clone()),
        other => Value::Json(other.clone()),
    })
}
