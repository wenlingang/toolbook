//! `js`: a JavaScript expression over earlier blocks, each as `$name`.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use regex::Regex;

use rquickjs::{CatchResultExt, Context, Runtime};

use super::{Tool, ToolCtx, ToolError};
use crate::value::Value;

pub(super) struct Js;

/// The memory a run may use: a base, and room for its inputs (QuickJS needs
/// about ten times a value's JSON size), up to a cap.
const MEMORY_BASE: usize = 64 * 1024 * 1024;
const MEMORY_PER_INPUT_BYTE: usize = 16;
const MEMORY_CAP: usize = 1024 * 1024 * 1024;
const TIME: Duration = Duration::from_secs(1);

/// "cannot read property 'x' of null（第 1 行第 1 列）": the message and where in the block it
/// happened, without the engine's stack (`at <eval> (eval_script:1:1)`).
fn tidy(error: &str) -> String {
    static WHERE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"eval_script:(\d+):(\d+)").unwrap());
    let first = error.lines().next().unwrap_or("").trim();
    let message = match first.split_once(": ") {
        Some((kind, rest)) if kind.ends_with("Error") && !kind.contains(' ') => rest,
        _ => first,
    };
    match WHERE.captures(error) {
        Some(c) => format!("{message}（第 {} 行第 {} 列）", &c[1], &c[2]),
        None => message.to_owned(),
    }
}

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
        let inputs: Vec<(&String, String)> = ctx
            .vars
            .iter()
            .filter(|(name, _)| is_ident(name))
            .map(|(name, v)| (name, v.to_json().to_string()))
            .collect();
        let input_bytes: usize = inputs.iter().map(|(_, j)| j.len()).sum();
        let memory = MEMORY_BASE
            .saturating_add(input_bytes.saturating_mul(MEMORY_PER_INPUT_BYTE))
            .min(MEMORY_CAP);
        let too_big = move || {
            ToolError::Failed(format!("JS 内存不够（上限 {} MB），输入太大", memory / 1024 / 1024))
        };
        let rt = Runtime::new().map_err(fail)?;
        rt.set_memory_limit(memory);
        let deadline = Instant::now() + TIME;
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
        let context = Context::full(&rt).map_err(fail)?;
        context.with(|js| {
            let globals = js.globals();
            for (name, json) in inputs {
                let value = js.json_parse(json).map_err(|e| match e {
                    rquickjs::Error::Exception => too_big(),
                    e => ToolError::Failed(format!("${name} 传给 JS 时出错：{e}")),
                })?;
                globals.set(format!("${name}"), value).map_err(fail)?;
            }
            let result: rquickjs::Value = js.eval(ctx.content.as_str()).catch(&js).map_err(|e| {
                if Instant::now() > deadline {
                    ToolError::Failed("JS 执行超过 1 秒，已停止".into())
                } else if let rquickjs::CaughtError::Value(v) = &e {
                    // `throw 42`, `throw "坏了"`: a thrown value, not an Error.
                    let shown = match v.as_string() {
                        Some(s) => s.to_string().unwrap_or_default(),
                        None => js
                            .json_stringify(v.clone())
                            .ok()
                            .flatten()
                            .and_then(|s| s.to_string().ok())
                            .unwrap_or_else(|| "undefined".into()),
                    };
                    ToolError::Failed(format!("JS 出错：{shown}"))
                } else {
                    ToolError::Failed(format!("JS 出错：{}", tidy(&e.to_string())))
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
