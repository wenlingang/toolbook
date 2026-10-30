//! `base64 mode=decode|encode`.

use base64::alphabet;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::{DecodeError, Engine};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Base64;

fn engine(url_safe: bool) -> GeneralPurpose {
    let config = GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    GeneralPurpose::new(if url_safe { &alphabet::URL_SAFE } else { &alphabet::STANDARD }, config)
}

/// Decodes standard or URL-safe Base64, padded or not. A string that is
/// only too short is incomplete, not wrong.
pub(crate) fn decode(input: &str) -> Result<Vec<u8>, ToolError> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Err(ToolError::Incomplete("输入为空".into()));
    }
    let url_safe = compact.contains(['-', '_']);
    engine(url_safe).decode(&compact).map_err(|e| match e {
        DecodeError::InvalidLength(_) | DecodeError::InvalidLastSymbol(..) => {
            ToolError::Incomplete("输入不完整".into())
        }
        DecodeError::InvalidByte(at, b) => ToolError::Failed(format!(
            "第 {} 个字符 {:?} 不是合法的 Base64 字符",
            at + 1,
            b as char
        )),
        DecodeError::InvalidPadding => ToolError::Failed("Base64 的 = 填充位置不对".into()),
    })
}

impl Tool for Base64 {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        if ctx.attr_text("mode").as_deref() == Some("encode") {
            return Ok(Value::Text(engine(false).encode(text.as_bytes())));
        }
        let bytes = decode(&text)?;
        let n = bytes.len();
        String::from_utf8(bytes)
            .map(Value::Text)
            .map_err(|_| ToolError::Failed(format!("解码结果不是 UTF-8 文本（共 {n} 字节）")))
    }
}
