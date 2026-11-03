//! `flow src=… out=… <输入>=…`: runs another note with these inputs.

use std::path::{Path, PathBuf};

use super::{Tool, ToolCtx, ToolError};
use crate::doc::Document;
use crate::engine::{BlockStatus, run_with_stack};
use crate::value::Value;

pub(super) struct Flow;

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl Tool for Flow {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let src = ctx
            .attr_text("src")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 src=笔记路径".into()))?;
        let found: Option<PathBuf> = [ctx.base_dir.join(&src), ctx.opts.library.join(&src)]
            .into_iter()
            .find(|p| p.is_file());
        let path = found.ok_or_else(|| ToolError::Failed(format!("找不到子流程 {src}")))?;
        let path = path.canonicalize().unwrap_or(path);
        if ctx.stack.contains(&path) {
            let mut chain: Vec<String> = ctx.stack.iter().map(|p| file_name(p)).collect();
            chain.push(file_name(&path));
            return Err(ToolError::Failed(format!("递归调用：{}", chain.join(" → "))));
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ToolError::Failed(format!("读取子流程 {src} 失败：{e}")))?;
        let doc = Document::parse(&text);

        let accepted: Vec<String> = crate::describe::describe(&doc).inputs.into_iter().map(|i| i.name).collect();
        let mut opts = ctx.opts.clone();
        opts.overrides.clear();
        for (k, v) in &ctx.attrs {
            if k != "src" && k != "out" {
                if !accepted.contains(k) {
                    return Err(ToolError::Failed(format!(
                        "子流程 {} 没有名为 {k} 的输入，可用：{}",
                        file_name(&path),
                        if accepted.is_empty() { "（无）".to_owned() } else { accepted.join("、") }
                    )));
                }
                let s = match v {
                    Value::Text(t) => t.clone(),
                    other => other.to_json().to_string(),
                };
                opts.overrides.insert(k.clone(), s);
            }
        }
        let mut stack = ctx.stack.to_vec();
        stack.push(path.clone());
        let base = path.parent().unwrap_or(Path::new("."));
        let report = run_with_stack(&doc, &opts, base, &stack);
        // What the note's secrets resolved to, its own flows' included: the
        // result may carry them.
        for b in &report.blocks {
            if b.tool == "secret"
                && let BlockStatus::Ok(Value::Text(v)) = &b.status
                && !v.is_empty()
            {
                ctx.secrets.borrow_mut().push(v.clone());
            }
            ctx.secrets.borrow_mut().extend(b.secrets.iter().cloned());
        }

        let out = ctx.attr_text("out");
        let result = report.output(out.as_deref()).ok_or_else(|| {
            ToolError::Failed(match &out {
                Some(o) => format!("子流程 {} 里没有名为 {o} 的块", file_name(&path)),
                None => format!("子流程 {} 里没有具名块可作为输出", file_name(&path)),
            })
        })?;
        match &result.status {
            BlockStatus::Ok(v) => Ok(v.clone()),
            _ => {
                let (i, why) = report.root_cause(result.index);
                Err(ToolError::Failed(format!("子流程 {} 的 [{}] 出错：{why}", file_name(&path), i + 1)))
            }
        }
    }
}
