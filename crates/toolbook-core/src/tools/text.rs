//! `text`: the block's content, or its `input=`, as it is.

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Text;

impl Tool for Text {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        Ok(ctx.require_primary()?.clone())
    }
}
