//! `hash algo=sha256|sha1|md5|sha512`.

use sha2::Digest;

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Hash;

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Tool for Hash {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        let data = text.as_bytes();
        let algo = ctx.attr_text("algo").unwrap_or_else(|| "sha256".into());
        let digest = match algo.as_str() {
            "sha256" => sha2::Sha256::digest(data).to_vec(),
            "sha512" => sha2::Sha512::digest(data).to_vec(),
            "sha1" => sha1::Sha1::digest(data).to_vec(),
            "md5" => md5::Md5::digest(data).to_vec(),
            other => {
                return Err(ToolError::Failed(format!(
                    "不支持的算法 {other}，可选 sha256、sha512、sha1、md5"
                )));
            }
        };
        Ok(Value::Text(hex(&digest)))
    }
}
