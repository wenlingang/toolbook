//! `js`: a JavaScript expression over earlier blocks, each as `$name`.

use std::time::{Duration, Instant};

use rquickjs::{CatchResultExt, Context, Runtime};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Js;

const MEMORY: usize = 32 * 1024 * 1024;
const TIME: Duration = Duration::from_secs(1);

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_') && chars.all(|c| c.is_alphanumeric() || c == '_')
}

impl Tool for Js {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        if ctx.content.trim().is_empty() {
            return Err(ToolError::Incomplete("输入为空".into()));
        }
        let fail = |e: rquickjs::Error| ToolError::Failed(format!("JS 运行环境出错：{e}"));
        let rt = Runtime::new().map_err(fail)?;
        rt.set_memory_limit(MEMORY);
        let deadline = Instant::now() + TIME;
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
        let context = Context::full(&rt).map_err(fail)?;
        context.with(|js| {
            let globals = js.globals();
            for (name, v) in &ctx.vars {
                if !is_ident(name) {
                    continue;
                }
                let value = js
                    .json_parse(v.to_json().to_string())
                    .map_err(|e| ToolError::Failed(format!("${name} 传给 JS 时出错：{e}")))?;
                globals.set(format!("${name}"), value).map_err(fail)?;
            }
            let result: rquickjs::Value = js.eval(ctx.content.as_str()).catch(&js).map_err(|e| {
                if Instant::now() > deadline {
                    ToolError::Failed("JS 执行超过 1 秒，已停止".into())
                } else {
                    ToolError::Failed(format!("JS 出错：{e}"))
                }
            })?;
            if let Some(s) = result.as_string() {
                return Ok(Value::Text(s.to_string().map_err(fail)?));
            }
            let json = js
                .json_stringify(result)
                .map_err(fail)?
                .ok_or_else(|| ToolError::Failed("JS 表达式没有返回值".into()))?
                .to_string()
                .map_err(fail)?;
            serde_json::from_str(&json)
                .map(Value::Json)
                .map_err(|e| ToolError::Failed(format!("JS 结果不是 JSON：{e}")))
        })
    }
}
