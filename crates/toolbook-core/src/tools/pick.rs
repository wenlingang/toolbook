//! `pick path=…`: a field by dotted path (`a.b[0]`) or JSONPath (`$.a[*].b`).

use serde_json::Value as Json;
use serde_json_path::JsonPath;

use super::{Tool, ToolCtx, ToolError};
use crate::path::{get_path, parse_path};
use crate::value::Value;

pub(super) struct Pick;

impl Tool for Pick {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let input = ctx.require_primary()?;
        let path = ctx
            .attr_text("path")
            .filter(|p| !p.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 path=".into()))?;
        if !path.starts_with('$') {
            let segs = parse_path(&path).map_err(ToolError::Failed)?;
            return get_path(input, &segs).map_err(ToolError::Failed);
        }
        let jp = JsonPath::parse(&path).map_err(|e| ToolError::Failed(format!("JSONPath 写法有误：{e}")))?;
        let root = match input {
            Value::Text(s) => super::json::parse(s)?,
            other => other.to_json(),
        };
        let found: Vec<Json> = jp.query(&root).all().into_iter().cloned().collect();
        let out = match found.len() {
            0 => return Err(ToolError::Failed(format!("{path} 没有匹配到任何值"))),
            1 => found.into_iter().next().unwrap_or(Json::Null),
            _ => Json::Array(found),
        };
        Ok(match out {
            Json::String(s) => Value::Text(s),
            other => Value::Json(other),
        })
    }
}
