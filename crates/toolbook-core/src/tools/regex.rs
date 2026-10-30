//! `regex pattern=… mode=extract|replace replace=…`.

use regex::Regex;
use serde_json::{Value as Json, json};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct RegexTool;

impl Tool for RegexTool {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        let pattern = ctx
            .attr_text("pattern")
            .filter(|p| !p.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 pattern=".into()))?;
        let re = Regex::new(&pattern).map_err(|e| ToolError::Failed(format!("正则有误：{e}")))?;
        if ctx.attr_text("mode").as_deref() == Some("replace") {
            let with = ctx.attr_text("replace").unwrap_or_default();
            return Ok(Value::Text(re.replace_all(&text, with.as_str()).into_owned()));
        }
        let groups = re.captures_len() > 1;
        let found: Vec<Json> = re
            .captures_iter(&text)
            .map(|c| {
                if groups {
                    Json::Array(
                        c.iter().skip(1).map(|m| m.map(|m| json!(m.as_str())).unwrap_or(Json::Null)).collect(),
                    )
                } else {
                    json!(c.get(0).map(|m| m.as_str()).unwrap_or(""))
                }
            })
            .collect();
        Ok(Value::Json(Json::Array(found)))
    }
}
