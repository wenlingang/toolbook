//! `secret`: a value kept out of the file, from the environment or the run.

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Secret;

/// `TOOLBOOK_SECRET_<NAME>`, with `-` as `_`.
pub(crate) fn env_key(name: &str) -> String {
    format!("TOOLBOOK_SECRET_{}", name.to_uppercase().replace('-', "_"))
}

impl Tool for Secret {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let Some(name) = &ctx.block.info.name else {
            return Err(ToolError::Failed("secret 块需要 name=".into()));
        };
        let key = env_key(name);
        ctx.opts
            .env
            .get(&key)
            .or_else(|| ctx.opts.overrides.get(name))
            .map(|v| Value::Text(v.clone()))
            .ok_or_else(|| {
                ToolError::Incomplete(format!(
                    "缺少密钥：设置环境变量 {key} 或传入 --input {name}=…"
                ))
            })
    }
}
