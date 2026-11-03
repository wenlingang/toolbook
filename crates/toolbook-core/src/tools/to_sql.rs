//! `to-sql table=… mode=insert|update key=…`: rows as SQL statements.

use serde_json::Value as Json;

use super::rows::records;
use super::{Tool, ToolCtx, ToolError};
use crate::fence::VarRef;
use crate::path::get_path;
use crate::template::render_template;
use crate::value::Value;

pub(super) struct ToSql;

/// `v` as a SQL literal. MySQL also reads a backslash as an escape, so
/// there it is doubled too: a value ending in `\` cannot end the string
/// early.
fn literal(v: &Json, dialect: &str) -> String {
    let quote = |s: &str| {
        let s = if dialect == "mysql" { s.replace('\\', "\\\\") } else { s.to_owned() };
        format!("'{}'", s.replace('\'', "''"))
    };
    match v {
        Json::Null => "NULL".into(),
        Json::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Json::Number(n) => n.to_string(),
        Json::String(s) => quote(s),
        other => quote(&other.to_string()),
    }
}

/// Words that cannot stand bare as a table or column name.
const RESERVED: &[&str] = &[
    "add", "all", "alter", "and", "as", "asc", "between", "by", "case", "check", "column", "create",
    "database", "default", "delete", "desc", "distinct", "drop", "else", "end", "exists", "from",
    "group", "having", "in", "index", "insert", "into", "is", "join", "key", "like", "limit", "not",
    "null", "on", "or", "order", "primary", "references", "select", "set", "table", "then", "to",
    "union", "unique", "update", "user", "values", "when", "where",
];

/// `name` as SQL: bare when it is a plain word, quoted for the dialect
/// otherwise (backticks for MySQL, the default; double quotes for
/// PostgreSQL and SQLite).
fn ident(name: &str, dialect: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !RESERVED.contains(&name.to_ascii_lowercase().as_str());
    if plain {
        return name.to_owned();
    }
    match dialect {
        "postgres" | "sqlite" => format!("\"{}\"", name.replace('"', "\"\"")),
        _ => format!("`{}`", name.replace('`', "``")),
    }
}

/// The content as a SQL template, once per row: `{{ 列 }}` (or `{{ 列.path }}`)
/// becomes that row's value as a SQL literal.
fn fill_template(ctx: &ToolCtx, dialect: &str) -> Result<Value, ToolError> {
    let input = ctx
        .attrs
        .get("input")
        .ok_or_else(|| ToolError::Incomplete("模板要配 input=$表格，按每一行填一次".into()))?;
    let rows = records(input)?;
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let lookup = |r: &VarRef| -> Result<Value, String> {
            let cell = row
                .get(&r.name)
                .ok_or_else(|| format!("第 {} 行没有列 {}，现有列：{}", i + 1, r.name, row.keys().cloned().collect::<Vec<_>>().join("、")))?;
            let v = get_path(&Value::Json(cell.clone()), &r.path)?.to_json();
            Ok(Value::Text(literal(&v, dialect)))
        };
        out.push(render_template(&ctx.content, &lookup).map_err(ToolError::Failed)?);
    }
    Ok(Value::Text(out.join("\n")))
}

impl Tool for ToSql {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
        let dialect = ctx.attr_text("dialect").unwrap_or_else(|| "mysql".into());
        if !matches!(dialect.as_str(), "mysql" | "postgres" | "sqlite") {
            return Err(ToolError::Failed(format!("dialect 只能是 mysql、postgres、sqlite，收到 {dialect}")));
        }
        if !ctx.content.trim().is_empty() {
            return fill_template(ctx, &dialect);
        }
        let rows = records(ctx.require_primary()?)?;
        let table = ctx
            .attr_text("table")
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ToolError::Incomplete("请设置 table=表名".into()))?;
        let update = match ctx.attr_text("mode").as_deref() {
            None | Some("insert") => false,
            Some("update") => true,
            Some(m) => return Err(ToolError::Failed(format!("mode 只能是 insert 或 update，收到 {m}"))),
        };
        let q = |n: &str| ident(n, &dialect);
        let table = q(&table);
        let mut out = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            if update {
                let key = ctx
                    .attr_text("key")
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| ToolError::Incomplete("update 需要设置 key=主键列".into()))?;
                let k = row
                    .get(&key)
                    .ok_or_else(|| ToolError::Failed(format!("第 {} 行没有主键列 {key}", i + 1)))?;
                let sets: Vec<String> = row
                    .iter()
                    .filter(|(c, _)| **c != key)
                    .map(|(c, v)| format!("{} = {}", q(c), literal(v, &dialect)))
                    .collect();
                out.push(format!("UPDATE {table} SET {} WHERE {} = {};", sets.join(", "), q(&key), literal(k, &dialect)));
            } else {
                let cols: Vec<String> = row.keys().map(|c| q(c)).collect();
                let vals: Vec<String> = row.values().map(|v| literal(v, &dialect)).collect();
                out.push(format!("INSERT INTO {table} ({}) VALUES ({});", cols.join(", "), vals.join(", ")));
            }
        }
        Ok(Value::Text(out.join("\n")))
    }
}
