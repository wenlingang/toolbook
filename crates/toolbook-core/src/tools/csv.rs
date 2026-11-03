//! `csv delimiter=, dedupe=列 require="列~正则;…"`: text to a table.

use std::collections::HashMap;

use regex::Regex;

use super::{Tool, ToolCtx, ToolError};
use crate::value::{Table, Value};

pub(super) struct Csv;

impl Tool for Csv {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let text = ctx.primary_text()?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        if text.trim().is_empty() {
            return Err(ToolError::Incomplete("输入为空".into()));
        }
        let delimiter = match ctx.attr_text("delimiter").as_deref() {
            None | Some("") => b',',
            Some("tab" | "\\t") => b'\t',
            Some(d) if d.len() == 1 => d.as_bytes()[0],
            Some(d) => return Err(ToolError::Failed(format!("delimiter 需要是单个字符，收到 {d:?}"))),
        };
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(text.as_bytes());
        let columns: Vec<String> = reader
            .headers()
            .map_err(|e| ToolError::Failed(format!("CSV 表头读不出来：{e}")))?
            .iter()
            .map(|h| h.trim().to_owned())
            .collect();
        let col = |name: &str| {
            columns
                .iter()
                .position(|c| c == name)
                .ok_or_else(|| ToolError::Failed(format!("没有列 {name}，现有列：{}", columns.join("、"))))
        };
        let mut requires = Vec::new();
        for rule in ctx.attr_text("require").unwrap_or_default().split(';').filter(|r| !r.trim().is_empty()) {
            let (name, pat) = rule
                .split_once('~')
                .ok_or_else(|| ToolError::Failed(format!("require 里的 {rule:?} 应写成 列~正则")))?;
            let re = Regex::new(pat).map_err(|e| ToolError::Failed(format!("正则 {pat:?} 有误：{e}")))?;
            requires.push((col(name.trim())?, name.trim().to_owned(), re, pat.to_owned()));
        }
        let dedupe = match ctx.attr_text("dedupe").filter(|d| !d.is_empty()) {
            Some(d) => Some(col(&d)?),
            None => None,
        };

        let mut rows = Vec::new();
        let mut seen: HashMap<String, u64> = HashMap::new();
        for rec in reader.records() {
            // Lines as in the text: a quoted field may span several.
            let rec = rec.map_err(|e| match e.position() {
                Some(p) => ToolError::Failed(format!("CSV 第 {} 行：{e}", p.line())),
                None => ToolError::Failed(format!("CSV 读不出来：{e}")),
            })?;
            let line = rec.position().map_or(0, |p| p.line());
            let mut row: Vec<String> = rec.iter().map(str::to_owned).collect();
            if row.len() > columns.len() {
                let extra = &row[columns.len()..];
                ctx.note(format!("第 {line} 行：比表头多 {} 个字段（{}），已去掉", extra.len(), extra.join("、")));
            }
            row.resize(columns.len(), String::new());
            if let Some((_, name, _, pat)) = requires.iter().find(|(c, _, re, _)| !re.is_match(&row[*c])) {
                ctx.note(format!("第 {line} 行：{name} 不匹配 {pat}，已去掉"));
                continue;
            }
            if let Some(c) = dedupe {
                if let Some(first) = seen.get(&row[c]) {
                    ctx.note(format!("第 {line} 行：与第 {first} 行重复，已去掉"));
                    continue;
                }
                seen.insert(row[c].clone(), line);
            }
            rows.push(row);
        }
        Ok(Value::Table(Table { columns, rows }))
    }
}
