//! `jwt-decode`: finds a JWT in its input and reads its header and payload.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value as Json, json};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct JwtDecode;

static TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*").unwrap());

fn part(s: &str, which: &str) -> Result<Json, ToolError> {
    let bytes = super::base64::decode(s)
        .map_err(|_| ToolError::Failed(format!("JWT 的 {which} 不是合法的 Base64")))?;
    serde_json::from_slice(&bytes).map_err(|_| ToolError::Failed(format!("JWT 的 {which} 不是 JSON")))
}

impl Tool for JwtDecode {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        // The first x.y.z whose header reads as JSON: a hostname before
        // the token (`Host: api.example.com`) is skipped.
        let mut first_error = None;
        for m in TOKEN.find_iter(&text) {
            let mut parts = m.as_str().split('.');
            let decoded = part(parts.next().unwrap_or(""), "header")
                .and_then(|h| part(parts.next().unwrap_or(""), "payload").map(|p| (h, p)));
            match decoded {
                Ok((header, payload)) => return Ok(Value::Json(json!({ "header": header, "payload": payload }))),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        Err(first_error.unwrap_or_else(|| ToolError::Incomplete("没有找到 JWT（格式应为 xxx.yyy.zzz）".into())))
    }
}
