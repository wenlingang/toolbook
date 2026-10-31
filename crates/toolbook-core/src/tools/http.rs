//! `http`: `METHOD URL`, header lines, a blank line, the body. Run by hand
//! in the app; `{{ name }}` in it is filled in.

use std::time::Duration;

use serde_json::{Map, Value as Json, json};

use super::{Tool, ToolCtx, ToolError};
use crate::fence::FenceInfo;
use crate::value::Value;

/// How a request is written.
const HOW: &str = "请求写在块里：第一行如 POST https://…，下面是请求头，空一行后是请求体，用 {{ params }} 引用前面的块";

pub(super) struct Http;

impl Tool for Http {
    fn manual(&self) -> bool {
        true
    }

    fn template(&self) -> bool {
        true
    }

    fn check(&self, info: &FenceInfo, content: &str) -> Result<(), ToolError> {
        // The request is the block's text, not settings: `body=`, `input=`
        // would be dropped without a word.
        if !info.attrs.is_empty() {
            let keys: Vec<&str> = info.attrs.iter().map(|(k, _)| k.as_str()).collect();
            return Err(ToolError::Failed(format!("http 块不认设置 {}：{HOW}", keys.join("、"))));
        }
        if content.trim().is_empty() {
            return Err(ToolError::Incomplete(HOW.into()));
        }
        Ok(())
    }

    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let mut lines = ctx.content.lines().skip_while(|l| l.trim().is_empty());
        let first = lines.next().ok_or_else(|| ToolError::Incomplete(HOW.into()))?;
        let mut words = first.split_whitespace();
        let (method, url) = match (words.next(), words.next()) {
            (Some(m), Some(u)) => (m.to_uppercase(), u.to_owned()),
            (Some(u), None) => ("GET".to_owned(), u.to_owned()),
            _ => return Err(ToolError::Incomplete("请写上请求，如 GET https://…".into())),
        };
        let mut req = ureq::request(&method, &url).timeout(Duration::from_secs(30));
        let mut body_lines = Vec::new();
        let mut in_body = false;
        for l in lines {
            if in_body {
                body_lines.push(l);
            } else if l.trim().is_empty() {
                in_body = true;
            } else {
                let (k, v) = l
                    .split_once(':')
                    .ok_or_else(|| ToolError::Failed(format!("请求头 {l:?} 应写成 名字: 值")))?;
                req = req.set(k.trim(), v.trim());
            }
        }
        let body = body_lines.join("\n");
        let result = if body.trim().is_empty() { req.call() } else { req.send_string(&body) };
        let resp = match result {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(ToolError::Failed(format!("请求失败：{e}"))),
        };
        let status = resp.status();
        let mut headers = Map::new();
        for name in resp.headers_names() {
            if let Some(v) = resp.header(&name) {
                headers.insert(name, json!(v));
            }
        }
        let text = resp
            .into_string()
            .map_err(|e| ToolError::Failed(format!("读取响应失败：{e}")))?;
        let body = serde_json::from_str::<Json>(&text).unwrap_or(Json::String(text));
        Ok(Value::Json(json!({"status": status, "headers": headers, "body": body})))
    }
}
