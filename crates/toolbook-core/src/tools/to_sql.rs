//! `to-sql table=… mode=insert|update key=…`: rows as SQL statements.

use serde_json::Value as Json;

use super::rows::records;
use super::{Tool, ToolCtx, ToolError};
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

impl Tool for ToSql {
    fn run(&self, ctx: &ToolCtx) -> Result<Value, ToolError> {
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
        let dialect = ctx.attr_text("dialect").unwrap_or_else(|| "mysql".into());
        if !matches!(dialect.as_str(), "mysql" | "postgres" | "sqlite") {
            return Err(ToolError::Failed(format!("dialect 只能是 mysql、postgres、sqlite，收到 {dialect}")));
        }
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
