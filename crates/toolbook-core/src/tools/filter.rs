//! `filter where="<字段> <op> <值>"`: keeps the rows that match.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value as Json};

use super::rows::{cell_text, records, same_kind};
use super::{Tool, ToolCtx, ToolError};
use crate::value::{Table, Value};

pub(super) struct Filter;

static COND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(\S+?)\s*(==|!=|>=|<=|>|<|\scontains\s)\s*(.+?)\s*$").unwrap()
});

struct Cond {
    field: String,
    op: String,
    text: String,
    number: Option<f64>,
}

fn parse(s: &str) -> Result<Cond, ToolError> {
    let c = COND.captures(s).ok_or_else(|| {
        ToolError::Failed(format!(
            "条件 {s:?} 看不懂，写法如 duration > 100、city == \"深圳\"、name contains 张"
        ))
    })?;
    let raw = &c[3];
    let quoted = raw.len() >= 2 && (raw.starts_with('"') && raw.ends_with('"') || raw.starts_with('\'') && raw.ends_with('\''));
    let text = if quoted { raw[1..raw.len() - 1].to_owned() } else { raw.to_owned() };
    Ok(Cond {
        field: c[1].to_owned(),
        op: c[2].trim().to_owned(),
        number: if quoted { None } else { text.parse().ok() },
        text,
    })
}

fn matches(row: &Map<String, Json>, c: &Cond) -> bool {
    let Some(v) = row.get(&c.field) else { return false };
    let s = cell_text(v);
    if c.op == "contains" {
        return s.contains(&c.text);
    }
    if let (Some(n), Ok(x)) = (c.number, s.parse::<f64>()) {
        return match c.op.as_str() {
            "==" => x == n,
            "!=" => x != n,
            ">" => x > n,
            ">=" => x >= n,
            "<" => x < n,
            "<=" => x <= n,
            _ => false,
        };
    }
    match c.op.as_str() {
        "==" => s == c.text,
        "!=" => s != c.text,
        ">" => s > c.text,
        ">=" => s >= c.text,
        "<" => s < c.text,
        "<=" => s <= c.text,
        _ => false,
    }
}

impl Tool for Filter {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let input = ctx.require_primary()?;
        let cond = ctx
            .attr_text("where")
            .filter(|w| !w.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 where=，如 where=\"duration > 100\"".into()))?;
        let cond = parse(&cond)?;
        // An array of objects is filtered in place: only the kept rows are copied.
        if let Value::Json(Json::Array(items)) = input {
            let mut kept = Vec::new();
            for item in items {
                let Json::Object(row) = item else {
                    return Err(ToolError::Failed("数组里的每一项都需要是对象".into()));
                };
                if matches(row, &cond) {
                    kept.push(item.clone());
                }
            }
            return Ok(Value::Json(Json::Array(kept)));
        }
        let rows: Vec<_> = records(input)?.into_iter().filter(|r| matches(r, &cond)).collect();
        let columns = match input {
            Value::Table(Table { columns, .. }) => Some(columns.clone()),
            _ => None,
        };
        Ok(same_kind(input, rows, columns))
    }
}
