//! `url mode=decode|encode`: percent-encoding.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Url;

/// Everything but letters, digits and `-_.~`.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

impl Tool for Url {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        if ctx.attr_text("mode").as_deref() == Some("encode") {
            return Ok(Value::Text(utf8_percent_encode(&text, COMPONENT).to_string()));
        }
        let plus = text.replace('+', " ");
        percent_decode_str(&plus)
            .decode_utf8()
            .map(|s| Value::Text(s.into_owned()))
            .map_err(|_| ToolError::Failed("解码结果不是 UTF-8 文本".into()))
    }
}
