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
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (i, rec) in reader.records().enumerate() {
            let line = i + 2;
            let rec = rec.map_err(|e| ToolError::Failed(format!("CSV 第 {line} 行：{e}")))?;
            let mut row: Vec<String> = rec.iter().map(str::to_owned).collect();
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
