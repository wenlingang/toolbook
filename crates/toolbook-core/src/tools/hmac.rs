//! `hmac data=… key=… algo=sha256|sha1|sha512`: hex and Base64 together.

use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::json;

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct HmacTool;

fn mac<M: Mac + hmac::digest::KeyInit>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = <M as hmac::digest::KeyInit>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

impl Tool for HmacTool {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let data = match ctx.attr_text("data") {
            Some(d) => d,
            None => ctx.primary_text()?,
        };
        let key = ctx
            .attr_text("key")
            .filter(|k| !k.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 key=".into()))?;
        let algo = ctx.attr_text("algo").unwrap_or_else(|| "sha256".into());
        let bytes = match algo.as_str() {
            "sha256" => mac::<Hmac<sha2::Sha256>>(key.as_bytes(), data.as_bytes()),
            "sha512" => mac::<Hmac<sha2::Sha512>>(key.as_bytes(), data.as_bytes()),
            "sha1" => mac::<Hmac<sha1::Sha1>>(key.as_bytes(), data.as_bytes()),
            other => {
                return Err(ToolError::Failed(format!("不支持的算法 {other}，可选 sha256、sha512、sha1")));
            }
        };
        Ok(Value::Json(json!({
            "hex": super::hash::hex(&bytes),
            "base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
        })))
    }
}
