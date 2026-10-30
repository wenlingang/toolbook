//! `json mode=format|minify`: parses and checks JSON.

use serde_json::Value as Json;

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct JsonTool;

/// Parses JSON text, with where it went wrong.
pub(crate) fn parse(text: &str) -> Result<Json, ToolError> {
    if text.trim().is_empty() {
        return Err(ToolError::Incomplete("输入为空".into()));
    }
    serde_json::from_str(text).map_err(|e| {
        if e.is_eof() {
            ToolError::Incomplete(format!("JSON 还没写完（第 {} 行第 {} 列）", e.line(), e.column()))
        } else {
            ToolError::Failed(format!("JSON 第 {} 行第 {} 列：{e}", e.line(), e.column()))
        }
    })
}

impl Tool for JsonTool {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let json = match ctx.require_primary()? {
            Value::Text(s) => parse(s)?,
            other => other.to_json(),
        };
        if ctx.attr_text("mode").as_deref() == Some("minify") {
            Ok(Value::Text(json.to_string()))
        } else {
            Ok(Value::Json(json))
        }
    }
}
