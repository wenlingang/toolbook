//! `to-json`: a table (or JSON text) as a JSON array of objects.

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct ToJson;

impl Tool for ToJson {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        Ok(match ctx.require_primary()? {
            Value::Text(s) => Value::Json(super::json::parse(s)?),
            other => Value::Json(other.to_json()),
        })
    }
}
