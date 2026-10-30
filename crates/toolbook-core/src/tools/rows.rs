//! Rows as tools that work on records see them: objects keyed by column.

use serde_json::{Map, Value as Json};

use super::ToolError;
use crate::value::{Table, Value};

/// Records from an array of objects, a table, or text holding JSON.
pub(crate) fn records(v: &Value) -> Result<Vec<Map<String, Json>>, ToolError> {
    let json = match v {
        Value::Text(s) => serde_json::from_str(s)
            .map_err(|_| ToolError::Failed("输入需要是对象数组或表格".into()))?,
        other => other.to_json(),
    };
    let Json::Array(items) = json else {
        return Err(ToolError::Failed("输入需要是对象数组或表格".into()));
    };
    items
        .into_iter()
        .map(|i| match i {
            Json::Object(m) => Ok(m),
            _ => Err(ToolError::Failed("数组里的每一项都需要是对象".into())),
        })
        .collect()
}

/// A JSON value as a cell or a comparison operand.
pub(crate) fn cell_text(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Null => String::new(),
        other => other.to_string(),
    }
}

/// Keeps the input's kind: a table stays a table.
pub(crate) fn same_kind(input: &Value, rows: Vec<Map<String, Json>>, columns: Option<Vec<String>>) -> Value {
    match input {
        Value::Table(_) => {
            let columns = columns.unwrap_or_else(|| {
                rows.first().map(|r| r.keys().cloned().collect()).unwrap_or_default()
            });
            let rows = rows
                .iter()
                .map(|r| columns.iter().map(|c| r.get(c).map(cell_text).unwrap_or_default()).collect())
                .collect();
            Value::Table(Table { columns, rows })
        }
        _ => Value::Json(Json::Array(rows.into_iter().map(Json::Object).collect())),
    }
}
